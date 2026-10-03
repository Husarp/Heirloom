//! Serves the archive's files to the UI (PLAN.md §5.3): originals, a browser-friendly view of each image, and
//! thumbnails made on first use and kept in the cache. Only files inside the open archive folder are served.
//!
//! URL paths (after `http://heirloom.localhost/` in the app, or `/media/` in the browser bridge):
//! - `file/<path>`: the file as it is;
//! - `view/<path>`: an image the WebView can show (TIFF and other formats are converted to JPEG);
//! - `thumb/<size>/<path>`: a JPEG no bigger than `size` pixels (64–1024);
//! - `import/<size>/<n>`: the n-th file of the import being reviewed (size 0 = the file itself), so files can be
//!   previewed before they are copied into the archive. Only the files the user gave the import are served.
//!
//! `<path>` is relative to the archive folder, with each segment percent-encoded.

use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct Roots {
    pub archive: PathBuf,
    /// Where thumbnails are kept; none = made every time.
    pub thumbs: Option<PathBuf>,
}

/// The folders of the open archive, shared between the command handler and the file server.
#[derive(Debug, Clone, Default)]
pub struct MediaRoots(Arc<RwLock<Option<Roots>>>, Arc<RwLock<Vec<PathBuf>>>);

impl MediaRoots {
    pub fn set(&self, roots: Option<Roots>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = roots;
    }

    fn get(&self) -> Option<Roots> {
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
    let Some(shared) = roots.get() else { return Served::NotFound };
    let url_path = url_path.split(['?', '#']).next().unwrap_or("");
    if let Some(rest) = url_path.strip_prefix("import/") {
        let (size, n) = rest.split_once('/').unwrap_or(("", ""));
        let (Ok(size), Some(path)) = (size.parse::<u32>(), n.parse::<usize>().ok().and_then(|n| roots.import_file(n))) else { return Served::BadRequest };
        return match size {
            0 => view(&path),
            s if THUMB_SIZES.contains(&s) => thumb(&shared, &path, s),
            _ => Served::BadRequest,
        };
    }
    let roots = shared;
    let (kind, rest) = url_path.split_once('/').unwrap_or((url_path, ""));
    let result = match kind {
        "file" => resolve(&roots.archive, rest).map(|p| read_file(&p)),
        "view" => resolve(&roots.archive, rest).map(|p| view(&p)),
        "thumb" => {
            let (size, rest) = rest.split_once('/').unwrap_or(("", ""));
            match size.parse::<u32>() {
                Ok(size) if THUMB_SIZES.contains(&size) => resolve(&roots.archive, rest).map(|p| thumb(&roots, &p, size)),
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

/// The archive-relative path as a real path inside the archive; None when it tries to leave the folder.
fn resolve(root: &Path, encoded: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for segment in encoded.split('/').filter(|s| !s.is_empty()) {
        let segment = percent_decode(segment)?;
        // A colon names a stream of a file (`x.jpg:ukryty`) or a drive, and a device name (`NUL`, `COM1.jpg`) opens a
        // device: neither is a file of the archive.
        if segment.contains(':') || is_device_name(&segment) {
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

/// A name Windows keeps for a device, with any extension: `CON`, `nul.txt`, `COM1.jpg`, `LPT¹`.
fn is_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ').to_ascii_uppercase();
    let numbered = |prefix: &str| stem.strip_prefix(prefix).is_some_and(|n| matches!(n, "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"));
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") || numbered("COM") || numbered("LPT")
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

fn thumb(roots: &Roots, path: &Path, size: u32) -> Served {
    let Ok(meta) = std::fs::metadata(path) else { return Served::NotFound };
    let cached = roots.thumbs.as_ref().map(|dir| {
        let modified = meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
        let key = blake3::hash(format!("{}|{}|{modified}", path.display(), meta.len()).as_bytes());
        dir.join(format!("{}-{size}.jpg", &key.to_hex()[..20]))
    });
    if let Some(body) = cached.as_ref().and_then(|p| std::fs::read(p).ok()) {
        return Served::Ok { body, mime: "image/jpeg" };
    }
    let Some(image) = load_image(path) else { return Served::NotFound };
    let served = encode_jpeg(&image.thumbnail(size, size));
    if let (Served::Ok { body, .. }, Some(target)) = (&served, &cached) {
        // The cache is only a speed-up: if it can't be written, the next request makes the thumbnail again.
        if let Some(dir) = target.parent() {
            let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(target, body));
        }
    }
    served
}

/// Decodes an image and turns it upright (phone photos store their rotation separately, in EXIF).
fn load_image(path: &Path) -> Option<DynamicImage> {
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
        roots.set(Some(Roots { archive: dir.join("a"), thumbs: Some(dir.join("thumbs")) }));
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
}
