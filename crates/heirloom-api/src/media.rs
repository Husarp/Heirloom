//! Serves the archive's files to the UI (PLAN.md §5.3): originals, a browser-friendly view of each image, and
//! thumbnails made on first use and kept in the cache. Only files inside the open archive folder are served.
//!
//! URL paths (after `http://heirloom.localhost/` in the app, or `/media/` in the browser bridge):
//! - `file/<path>`: the file as it is;
//! - `view/<path>`: an image the WebView can show (TIFF and other formats are converted to JPEG);
//! - `thumb/<size>/<path>`: a JPEG no bigger than `size` pixels (64–1024);
//! - `thumb/<size>/crop:L,T,W,H/<path>`: the same of a part of the upright image (a person's profile photo crop; pixels,
//!   W/H 0 = to the edge);
//! - `import/<size>/<n>`: the n-th file of the import being reviewed (size 0 = the file itself), so files can be
//!   previewed before they are copied into the archive. Only the files the user gave the import are served.
//!
//! `<path>` is relative to the archive folder, with each segment percent-encoded. In a set of archives opened together
//! („Otwórz razem…”) every path starts with `~<key>/`, the archive's key in the set: `thumb/256/~b/media/x.jpg`.

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct Roots {
    pub archive: PathBuf,
    /// „Nie ruszaj oryginału”: `archive` is the folder of the user's changes, and a file it doesn't have is served
    /// from the original archive's folder.
    pub fallback: Option<PathBuf>,
    /// Where thumbnails are kept; none = made every time.
    pub thumbs: Option<PathBuf>,
    /// The archive's key in a set of archives opened together; none for an archive opened on its own.
    pub key: Option<String>,
    /// Files someone moved in Explorer, found by the scan (Part B): served only after both folders miss.
    pub moved: Moved,
}

/// Where the scan found files whose written path is missing (FILE value → the file's real path). Only paths the walk
/// found under the archive's folders are in it, so it opens no way out of the archive. Shared between the command
/// handler (the scan writes it) and the file server.
#[derive(Debug, Clone, Default)]
pub struct Moved(Arc<RwLock<std::collections::HashMap<String, PathBuf>>>);

impl Moved {
    pub fn set(&self, map: std::collections::HashMap<String, PathBuf>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = map;
    }

    /// The real path of a file written as `file` (compared as [`moved_key`] does).
    pub fn get(&self, file: &str) -> Option<PathBuf> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).get(&moved_key(file)).cloned()
    }

    pub fn len(&self) -> usize {
        self.0.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A FILE value as the "moved" map keys it: `/` as the separator, NFC, lowercase.
pub fn moved_key(file: &str) -> String {
    crate::fsx::name_key(file.replace('\\', "/").trim_start_matches("./"))
}

/// The folders of the open archive (or of every archive of an open set), shared between the command handler and the
/// file server.
#[derive(Debug, Clone, Default)]
pub struct MediaRoots(Arc<RwLock<Vec<Roots>>>, Arc<RwLock<Vec<PathBuf>>>);

impl MediaRoots {
    pub fn set(&self, roots: Option<Roots>) {
        self.set_many(roots.into_iter().collect());
    }

    /// The archives of a set, each with its key.
    pub fn set_many(&self, roots: Vec<Roots>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = roots;
    }

    fn get(&self) -> Vec<Roots> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The files of the import being reviewed.
    pub fn set_imports(&self, files: Vec<PathBuf>) {
        *self.1.write().unwrap_or_else(|e| e.into_inner()) = files;
    }

    fn import_file(&self, n: usize) -> Option<PathBuf> {
        self.1.read().unwrap_or_else(|e| e.into_inner()).get(n).cloned()
    }
}

#[derive(Debug, PartialEq)]
pub enum Served {
    Ok { body: Vec<u8>, mime: &'static str },
    NotFound,
    BadRequest,
}

const THUMB_SIZES: [u32; 5] = [64, 128, 256, 512, 1024];
const VIEW_SIZE: u32 = 2560;

/// Answers one request path (without the leading `/`).
pub fn serve(roots: &MediaRoots, url_path: &str) -> Served {
    let all = roots.get();
    let Some(shared) = all.first().cloned() else { return Served::NotFound };
    let url_path = url_path.split(['?', '#']).next().unwrap_or("");
    if let Some(rest) = url_path.strip_prefix("import/") {
        let (size, n) = rest.split_once('/').unwrap_or(("", ""));
        let (Ok(size), Some(path)) = (size.parse::<u32>(), n.parse::<usize>().ok().and_then(|n| roots.import_file(n))) else { return Served::BadRequest };
        return match size {
            0 => view(&path),
            s if THUMB_SIZES.contains(&s) => thumb(&shared, &path, s, None),
            _ => Served::BadRequest,
        };
    }
    let (kind, rest) = url_path.split_once('/').unwrap_or((url_path, ""));
    // Which archive: the only one, or in a set the one named by the path's first segment (`~b`).
    // The file in the folder of the user's changes, else in the original archive (the same checks for both).
    let locate = |roots: &Roots, rest: &str| -> Option<PathBuf> {
        let path = resolve(&roots.archive, rest)?;
        let found = match &roots.fallback {
            Some(fallback) if !path.is_file() => Some(resolve(fallback, rest)?).filter(|p| p.is_file()).unwrap_or(path),
            _ => path,
        };
        // Both folders miss: a file someone moved in Explorer (the scan knows where it is).
        if !found.is_file() {
            if let Some(there) = decode_path(rest).and_then(|p| roots.moved.get(&p)).filter(|p| p.is_file()) {
                return Some(there);
            }
        }
        Some(found)
    };
    let pick = |rest: &str| -> Option<(Roots, PathBuf)> {
        if all.len() == 1 && shared.key.is_none() {
            return Some((shared.clone(), locate(&shared, rest)?));
        }
        let (first, rest) = rest.trim_start_matches('/').split_once('/')?;
        let key = first.strip_prefix('~')?;
        let roots = all.iter().find(|r| r.key.as_deref() == Some(key))?.clone();
        let path = locate(&roots, rest)?;
        Some((roots, path))
    };
    let result = match kind {
        "file" => pick(rest).map(|(_, p)| read_file(&p)),
        "view" => pick(rest).map(|(_, p)| view(&p)),
        "thumb" => {
            let (size, rest) = rest.split_once('/').unwrap_or(("", ""));
            match size.parse::<u32>() {
                Ok(size) if THUMB_SIZES.contains(&size) => {
                    let Ok((crop, rest)) = split_crop(rest) else { return Served::BadRequest };
                    pick(rest).map(|(roots, p)| thumb(&roots, &p, size, crop))
                }
                _ => return Served::BadRequest,
            }
        }
        _ => return Served::BadRequest,
    };
    match result {
        Some(served) => served,
        None => Served::BadRequest,
    }
}

/// A crop in pixels of the upright image: left, top, width, height (0 = to the edge).
type Crop = [u32; 4];

/// Takes a leading `crop:L,T,W,H` segment (percent-encoded) off the path; the path without it. A crop that isn't four
/// unsigned integers is an error.
fn split_crop(rest: &str) -> Result<(Option<Crop>, &str), ()> {
    let rest = rest.trim_start_matches('/');
    let (first, tail) = rest.split_once('/').unwrap_or((rest, ""));
    let Some(text) = percent_decode(first).and_then(|s| s.strip_prefix("crop:").map(str::to_string)) else { return Ok((None, rest)) };
    let numbers: Option<Vec<u32>> = text.split(',').map(|n| n.trim().parse().ok()).collect();
    match numbers.as_deref() {
        Some(&[left, top, width, height]) => Ok((Some([left, top, width, height]), tail)),
        _ => Err(()),
    }
}

/// The archive-relative path as a real path inside the archive; None when it tries to leave the folder.
fn resolve(root: &Path, encoded: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for segment in encoded.split('/').filter(|s| !s.is_empty()) {
        let segment = percent_decode(segment)?;
        // A colon names a stream of a file (`x.jpg:ukryty`) or a drive, and a device name (`NUL`, `COM1.jpg`) opens a
        // device: neither is a file of the archive.
        if segment.contains(':') || crate::fsx::is_device_name(&segment) {
            return None;
        }
        let component = Path::new(&segment).components().collect::<Vec<_>>();
        match component.as_slice() {
            [Component::Normal(name)] => path.push(name),
            _ => return None,
        }
    }
    (path != root).then_some(path)
}

/// The URL path (segments percent-encoded) as plain text with `/` between the segments.
fn decode_path(encoded: &str) -> Option<String> {
    let segments: Option<Vec<String>> = encoded.split('/').filter(|s| !s.is_empty()).map(percent_decode).collect();
    segments.map(|s| s.join("/"))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn read_file(path: &Path) -> Served {
    match std::fs::read(path) {
        Ok(body) => Served::Ok { body, mime: mime_for(path) },
        Err(_) => Served::NotFound,
    }
}

pub fn mime_for(path: &Path) -> &'static str {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain; charset=utf-8",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// Formats the WebView shows directly.
fn web_image(path: &Path) -> bool {
    matches!(mime_for(path), "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/bmp" | "image/avif" | "image/svg+xml")
}

fn view(path: &Path) -> Served {
    if !path.is_file() {
        return Served::NotFound;
    }
    if web_image(path) {
        return read_file(path);
    }
    match load_image(path) {
        Some(image) => encode_jpeg(&image.thumbnail(VIEW_SIZE, VIEW_SIZE)),
        None => read_file(path),
    }
}

fn thumb(roots: &Roots, path: &Path, size: u32, crop: Option<Crop>) -> Served {
    let Ok(meta) = std::fs::metadata(path) else { return Served::NotFound };
    let cached = roots.thumbs.as_ref().map(|dir| {
        let modified = meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
        let crop_key = crop.map(|c| format!("|crop{},{},{},{}", c[0], c[1], c[2], c[3])).unwrap_or_default();
        let key = blake3::hash(format!("{}|{}|{modified}{crop_key}", path.display(), meta.len()).as_bytes());
        dir.join(format!("{}-{size}.jpg", &key.to_hex()[..20]))
    });
    if let Some(body) = cached.as_ref().and_then(|p| std::fs::read(p).ok()) {
        return Served::Ok { body, mime: "image/jpeg" };
    }
    let Some(image) = load_image(path) else { return Served::NotFound };
    let image = match crop.and_then(|c| crop_rect(c, (image.width(), image.height()))) {
        Some((left, top, width, height)) => image.crop_imm(left, top, width, height),
        None => image,
    };
    let served = encode_jpeg(&image.thumbnail(size, size));
    if let (Served::Ok { body, .. }, Some(target)) = (&served, &cached) {
        // The cache is only a speed-up: if it can't be written, the next request makes the thumbnail again.
        if let Some(dir) = target.parent() {
            let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(target, body));
        }
    }
    served
}

/// The crop as (left, top, width, height) inside an image of the given size; width/height 0 = to the edge, a bigger
/// one is cut at the edge. None when it starts outside the image (the whole image is used then).
fn crop_rect([left, top, width, height]: Crop, (image_width, image_height): (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    if left >= image_width || top >= image_height {
        return None;
    }
    let (room_w, room_h) = (image_width - left, image_height - top);
    Some((left, top, if width == 0 { room_w } else { width.min(room_w) }, if height == 0 { room_h } else { height.min(room_h) }))
}

/// The size of an image as it is shown (after the EXIF rotation), read from the header without decoding the pixels.
pub(crate) fn upright_size(path: &Path) -> Option<(u32, u32)> {
    let mut decoder = ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_decoder().ok()?;
    let (width, height) = decoder.dimensions();
    let turned = matches!(
        decoder.orientation().unwrap_or(Orientation::NoTransforms),
        Orientation::Rotate90 | Orientation::Rotate270 | Orientation::Rotate90FlipH | Orientation::Rotate270FlipH
    );
    Some(if turned { (height, width) } else { (width, height) })
}

/// Decodes an image and turns it upright (phone photos store their rotation separately, in EXIF).
pub(crate) fn load_image(path: &Path) -> Option<DynamicImage> {
    let mut decoder = ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    Some(image)
}

fn encode_jpeg(image: &DynamicImage) -> Served {
    let mut body = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut body, 85);
    match image.to_rgb8().write_with_encoder(encoder) {
        Ok(()) => Served::Ok { body, mime: "image/jpeg" },
        Err(_) => Served::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(dir: &Path) -> MediaRoots {
        let roots = MediaRoots::default();
        roots.set(Some(Roots { archive: dir.join("a"), fallback: None, thumbs: Some(dir.join("thumbs")), key: None, moved: Moved::default() }));
        roots
    }

    #[test]
    fn serves_files_only_inside_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/media")).unwrap();
        std::fs::write(dir.path().join("a/media/Józef 1878.txt"), "tekst").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "nie").unwrap();
        let roots = roots(dir.path());
        assert_eq!(
            serve(&roots, "file/media/J%C3%B3zef%201878.txt"),
            Served::Ok { body: b"tekst".to_vec(), mime: "text/plain; charset=utf-8" }
        );
        assert_eq!(serve(&roots, "file/../secret.txt"), Served::BadRequest);
        assert_eq!(serve(&roots, "file/media/..%2F..%2Fsecret.txt"), Served::BadRequest);
        assert_eq!(serve(&roots, "file/C:%5Csecret.txt"), Served::BadRequest);
        assert_eq!(serve(&roots, "file/media/brak.jpg"), Served::NotFound);
        assert_eq!(serve(&roots, "thumb/77/media/x.png"), Served::BadRequest);
    }

    #[test]
    fn a_set_serves_each_archive_by_its_key() {
        let dir = tempfile::tempdir().unwrap();
        for (folder, text) in [("a", "z a"), ("b", "z b")] {
            std::fs::create_dir_all(dir.path().join(folder).join("media")).unwrap();
            std::fs::write(dir.path().join(folder).join("media/x.txt"), text).unwrap();
        }
        std::fs::write(dir.path().join("secret.txt"), "nie").unwrap();
        image::RgbImage::from_pixel(300, 300, image::Rgb([1, 2, 3])).save(dir.path().join("b/media/p.png")).unwrap();
        let roots = MediaRoots::default();
        roots.set_many(["a", "b"].iter().map(|k| Roots { archive: dir.path().join(k), fallback: None, thumbs: Some(dir.path().join("thumbs").join(k)), key: Some(k.to_string()), moved: Moved::default() }).collect());
        assert_eq!(serve(&roots, "file/~b/media/x.txt"), Served::Ok { body: b"z b".to_vec(), mime: "text/plain; charset=utf-8" });
        assert_eq!(serve(&roots, "file/~a/media/x.txt"), Served::Ok { body: b"z a".to_vec(), mime: "text/plain; charset=utf-8" });
        assert!(matches!(serve(&roots, "thumb/256/~b/media/p.png"), Served::Ok { mime: "image/jpeg", .. }));
        assert!(dir.path().join("thumbs/b").is_dir(), "each archive keeps its own thumbnails");
        for path in ["file/~b/../secret.txt", "file/~b/..%2F..%2Fsecret.txt", "file/~z/media/x.txt", "file/media/x.txt", "file/~b", "thumb/256/media/p.png"] {
            assert_eq!(serve(&roots, path), Served::BadRequest, "{path}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn streams_and_device_names_are_not_files_of_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/media")).unwrap();
        std::fs::write(dir.path().join("a/media/x.txt"), "tekst").unwrap();
        std::fs::write(dir.path().join("a/media/x.txt:ukryty"), "strumień").unwrap();
        let roots = roots(dir.path());
        for path in ["file/media/x.txt:ukryty", "file/media/x.txt%3Aukryty", "file/media/NUL", "view/nul.txt", "file/media/com1.jpg", "file/media/LPT%C2%B9"] {
            assert_eq!(serve(&roots, path), Served::BadRequest, "{path}");
        }
        assert!(matches!(serve(&roots, "file/media/x.txt"), Served::Ok { .. }));
    }

    #[test]
    fn the_original_archive_serves_what_the_changes_folder_lacks() {
        let dir = tempfile::tempdir().unwrap();
        for (folder, name, text) in [("zmiany", "nowe.txt", "moje"), ("oryginal", "stare.txt", "z archiwum"), ("oryginal", "nowe.txt", "stare")] {
            std::fs::create_dir_all(dir.path().join(folder).join("media")).unwrap();
            std::fs::write(dir.path().join(folder).join("media").join(name), text).unwrap();
        }
        std::fs::write(dir.path().join("secret.txt"), "nie").unwrap();
        let roots = MediaRoots::default();
        roots.set(Some(Roots { archive: dir.path().join("zmiany"), fallback: Some(dir.path().join("oryginal")), thumbs: None, key: None, moved: Moved::default() }));
        let text = |body: &str| Served::Ok { body: body.as_bytes().to_vec(), mime: "text/plain; charset=utf-8" };
        assert_eq!(serve(&roots, "file/media/nowe.txt"), text("moje"), "the user's file wins");
        assert_eq!(serve(&roots, "file/media/stare.txt"), text("z archiwum"));
        assert_eq!(serve(&roots, "file/media/brak.txt"), Served::NotFound);
        assert_eq!(serve(&roots, "file/../secret.txt"), Served::BadRequest);
        assert_eq!(serve(&roots, "file/media/..%2F..%2Fsecret.txt"), Served::BadRequest, "no way out through the original either");
    }

    #[test]
    fn thumbnails_are_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/media")).unwrap();
        image::RgbImage::from_pixel(600, 300, image::Rgb([200, 100, 50])).save(dir.path().join("a/media/p.png")).unwrap();
        let roots = roots(dir.path());
        let Served::Ok { body, mime } = serve(&roots, "thumb/128/media/p.png") else { panic!("no thumbnail") };
        assert_eq!(mime, "image/jpeg");
        let thumb = image::load_from_memory(&body).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (128, 64));
        assert_eq!(std::fs::read_dir(dir.path().join("thumbs")).unwrap().count(), 1);
        assert_eq!(serve(&roots, "thumb/128/media/p.png"), Served::Ok { body, mime: "image/jpeg" });
    }

    /// 400 × 200: the left half red, the right half blue.
    fn two_halves(dir: &Path) {
        std::fs::create_dir_all(dir.join("a/media")).unwrap();
        image::RgbImage::from_fn(400, 200, |x, _| if x < 200 { image::Rgb([230, 20, 20]) } else { image::Rgb([20, 20, 230]) }).save(dir.join("a/media/p.png")).unwrap();
    }

    fn decoded(served: Served) -> image::RgbImage {
        let Served::Ok { body, mime } = served else { panic!("not served: {served:?}") };
        assert_eq!(mime, "image/jpeg");
        image::load_from_memory(&body).unwrap().to_rgb8()
    }

    #[test]
    fn a_thumbnail_can_be_cut_from_a_part_of_the_photo() {
        let dir = tempfile::tempdir().unwrap();
        two_halves(dir.path());
        let roots = roots(dir.path());
        // Pixels 100–300 across the full height: red on the left, blue on the right, square.
        let cut = decoded(serve(&roots, "thumb/128/crop%3A100%2C0%2C200%2C200/media/p.png"));
        assert_eq!((cut.width(), cut.height()), (128, 128));
        let (left, right) = (cut.get_pixel(10, 64), cut.get_pixel(117, 64));
        assert!(left[0] > 150 && left[2] < 100, "red on the left: {left:?}");
        assert!(right[2] > 150 && right[0] < 100, "blue on the right: {right:?}");
        // Another crop is another picture (and another cached file); the plain thumbnail is untouched.
        let blue = decoded(serve(&roots, "thumb/128/crop%3A200%2C0%2C200%2C200/media/p.png"));
        assert!(blue.get_pixel(10, 64)[2] > 150 && blue.get_pixel(10, 64)[0] < 100, "all blue");
        assert_eq!(decoded(serve(&roots, "thumb/128/media/p.png")).dimensions(), (128, 64));
        assert_eq!(std::fs::read_dir(dir.path().join("thumbs")).unwrap().count(), 3);
        let again = decoded(serve(&roots, "thumb/128/crop%3A100%2C0%2C200%2C200/media/p.png"));
        assert_eq!(again.get_pixel(10, 64), cut.get_pixel(10, 64));
        assert_eq!(std::fs::read_dir(dir.path().join("thumbs")).unwrap().count(), 3, "the same crop is served from the cache");
        // 0 as the width or height goes to the edge; a crop reaching past the edge is cut there.
        assert_eq!(decoded(serve(&roots, "thumb/128/crop%3A200%2C0%2C0%2C0/media/p.png")).dimensions(), (128, 128));
        assert_eq!(decoded(serve(&roots, "thumb/128/crop%3A300%2C0%2C500%2C500/media/p.png")).dimensions(), (64, 128));
        // A crop outside the picture falls back to the whole picture.
        assert_eq!(decoded(serve(&roots, "thumb/128/crop%3A500%2C0%2C100%2C100/media/p.png")).dimensions(), (128, 64));
    }

    #[test]
    fn a_bad_crop_is_refused_and_only_thumbnails_take_one() {
        let dir = tempfile::tempdir().unwrap();
        two_halves(dir.path());
        let roots = roots(dir.path());
        for path in [
            "thumb/128/crop%3Aabc/media/p.png",
            "thumb/128/crop%3A1%2C2%2C3/media/p.png",
            "thumb/128/crop%3A1%2C2%2C3%2C4%2C5/media/p.png",
            "thumb/128/crop%3A-1%2C0%2C10%2C10/media/p.png",
            "thumb/128/crop%3A1%2C2%2C3%2C/media/p.png",
            "file/crop%3A100%2C0%2C200%2C200/media/p.png",
            "view/crop%3A100%2C0%2C200%2C200/media/p.png",
        ] {
            assert_eq!(serve(&roots, path), Served::BadRequest, "{path}");
        }
        assert_eq!(serve(&roots, "thumb/128/crop%3A1%2C2%2C3%2C4/media/brak.png"), Served::NotFound, "a good crop of a missing file is just missing");
    }

    #[test]
    fn a_crop_comes_before_the_archive_key_in_a_set() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("b/media")).unwrap();
        image::RgbImage::from_fn(400, 200, |x, _| if x < 200 { image::Rgb([230, 20, 20]) } else { image::Rgb([20, 20, 230]) }).save(dir.path().join("b/media/p.png")).unwrap();
        let roots = MediaRoots::default();
        roots.set_many(["a", "b"].iter().map(|k| Roots { archive: dir.path().join(k), fallback: None, thumbs: Some(dir.path().join("thumbs").join(k)), key: Some(k.to_string()), moved: Moved::default() }).collect());
        let cut = decoded(serve(&roots, "thumb/128/crop%3A200%2C0%2C200%2C200/~b/media/p.png"));
        assert_eq!((cut.width(), cut.height()), (128, 128));
        assert!(cut.get_pixel(64, 64)[2] > 150);
        assert_eq!(serve(&roots, "thumb/128/~b/crop%3A200%2C0%2C200%2C200/media/p.png"), Served::BadRequest, "the crop is the first segment");
        assert_eq!(serve(&roots, "thumb/128/crop%3A200%2C0%2C200%2C200/media/p.png"), Served::BadRequest, "a set needs the key");
    }

    /// A JPEG of the given size with an EXIF orientation tag.
    fn jpeg_with_orientation(width: u32, height: u32, orientation: u8) -> Vec<u8> {
        let mut plain = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut plain, 80);
        image::RgbImage::from_pixel(width, height, image::Rgb([90, 90, 90])).write_with_encoder(encoder).unwrap();
        // APP1 "Exif": a little-endian TIFF header and one IFD0 entry, Orientation (0x0112, SHORT, 1).
        let mut exif = b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        exif.extend([orientation, 0, 0, 0, 0, 0, 0, 0]);
        let mut out = plain[..2].to_vec();
        out.extend([0xFF, 0xE1]);
        out.extend(((exif.len() + 2) as u16).to_be_bytes());
        out.extend(exif);
        out.extend(&plain[2..]);
        out
    }

    #[test]
    fn the_upright_size_follows_the_exif_rotation() {
        let dir = tempfile::tempdir().unwrap();
        for (orientation, size) in [(1, (40, 20)), (3, (40, 20)), (6, (20, 40)), (8, (20, 40)), (5, (20, 40)), (7, (20, 40))] {
            let file = dir.path().join(format!("o{orientation}.jpg"));
            std::fs::write(&file, jpeg_with_orientation(40, 20, orientation)).unwrap();
            assert_eq!(upright_size(&file), Some(size), "orientation {orientation}");
            let upright = load_image(&file).unwrap();
            assert_eq!((upright.width(), upright.height()), size, "the same as the decoded image");
        }
        image::RgbImage::from_pixel(30, 10, image::Rgb([1, 2, 3])).save(dir.path().join("plain.png")).unwrap();
        assert_eq!(upright_size(&dir.path().join("plain.png")), Some((30, 10)));
        std::fs::write(dir.path().join("text.png"), "not an image").unwrap();
        assert_eq!(upright_size(&dir.path().join("text.png")), None);
        assert_eq!(upright_size(&dir.path().join("none.png")), None);
    }
}
