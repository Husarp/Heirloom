//! Photo edits (turn, straighten, trim) as new versions of the file; the original is kept.
//!
//! The edited picture is written as a new file next to the others (never replacing one), and the object record keeps
//! every version as a `FILE`: the first is the one shown, the older ones follow, marked `_HLM_VERSION original` (the
//! photo as it came) or `earlier` (a version that was replaced by a newer edit).

use crate::edit::Changes;
use crate::gedwrite::{file_uri, media_type};
use crate::{ApiError, ApiResult, Session};
use heirloom_core::gedcom::Node;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgba, RgbaImage};
use serde_json::{Value, json};

/// Smaller turns than this (in degrees) are not a straightening.
const MIN_ANGLE: f64 = 0.01;
/// The straighten slider's range, in degrees.
const MAX_ANGLE: f64 = 15.0;
const JPEG_QUALITY: u8 = 92;

pub fn call(s: &mut Session, method: &str, args: &Value) -> ApiResult {
    match method {
        "media.editPhoto" => edit_photo(s, args),
        "media.restoreOriginal" => restore_original(s, args),
        _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
    }
}

// ---------- pixels ----------

/// 0–3 quarter turns clockwise.
pub fn rotate_quarter(img: &DynamicImage, quarter: u8) -> DynamicImage {
    match quarter % 4 {
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => img.clone(),
    }
}

/// The scale of the largest centred rectangle with the image's own aspect that has no empty corners once the image is
/// turned by `degrees` (the same formula the editor's preview uses).
pub fn straighten_scale(width: f64, height: f64, degrees: f64) -> f64 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (c, s) = (cos.abs(), sin.abs());
    (width / (width * c + height * s)).min(height / (width * s + height * c))
}

/// Turns the image about its centre (clockwise for positive degrees) and keeps the largest centred rectangle of the
/// image's own aspect that has no empty corners.
pub fn straighten(img: &DynamicImage, degrees: f64) -> DynamicImage {
    if degrees.abs() < MIN_ANGLE {
        return img.clone();
    }
    let (width, height) = img.dimensions();
    let (w, h) = (f64::from(width), f64::from(height));
    let k = straighten_scale(w, h, degrees);
    let out_w = ((k * w).round() as u32).clamp(1, width);
    let out_h = ((k * h).round() as u32).clamp(1, height);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let source = img.to_rgba8();
    let mut out = RgbaImage::new(out_w, out_h);
    for (ox, oy, pixel) in out.enumerate_pixels_mut() {
        let dx = f64::from(ox) + 0.5 - f64::from(out_w) / 2.0;
        let dy = f64::from(oy) + 0.5 - f64::from(out_h) / 2.0;
        let sx = w / 2.0 + dx * cos + dy * sin - 0.5;
        let sy = h / 2.0 - dx * sin + dy * cos - 0.5;
        *pixel = sample(&source, sx, sy);
    }
    if img.color().has_alpha() { DynamicImage::ImageRgba8(out) } else { DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(out).to_rgb8()) }
}

/// Bilinear sample at (x, y) in pixel-centre coordinates; the edge pixels repeat outside the image.
fn sample(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (width, height) = img.dimensions();
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |px: f64, py: f64| img.get_pixel((px.max(0.0) as u32).min(width - 1), (py.max(0.0) as u32).min(height - 1)).0;
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
    let mut out = [0u8; 4];
    for i in 0..4 {
        let top = f64::from(a[i]) * (1.0 - fx) + f64::from(b[i]) * fx;
        let bottom = f64::from(c[i]) * (1.0 - fx) + f64::from(d[i]) * fx;
        out[i] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
    }
    Rgba(out)
}

/// The part of the image given in fractions (0–1) of its width and height, kept inside the image and at least 1×1 px.
pub fn crop_fraction(img: &DynamicImage, x: f64, y: f64, w: f64, h: f64) -> DynamicImage {
    let (width, height) = img.dimensions();
    let edge = |fraction: f64, size: u32| (fraction.clamp(0.0, 1.0) * f64::from(size)).round() as u32;
    let left = edge(x, width).min(width - 1);
    let top = edge(y, height).min(height - 1);
    let right = edge(x + w, width).clamp(left + 1, width);
    let bottom = edge(y + h, height).clamp(top + 1, height);
    img.crop_imm(left, top, right - left, bottom - top)
}

// ---------- the commands ----------

struct Crop {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A crop that covers the whole picture is no crop.
fn crop_arg(args: &Value) -> Result<Option<Crop>, ApiError> {
    let Some(crop) = args.get("crop").filter(|c| !c.is_null()) else { return Ok(None) };
    let number = |key: &str| crop.get(key).and_then(Value::as_f64).ok_or_else(|| ApiError::bad_args(format!("crop.{key}")));
    let crop = Crop { x: number("x")?, y: number("y")?, w: number("w")?, h: number("h")? };
    let whole = crop.x <= 0.0 && crop.y <= 0.0 && crop.x + crop.w >= 1.0 && crop.y + crop.h >= 1.0;
    Ok((!whole).then_some(crop))
}

const INSIDE_PERSON: &str = "Tego pliku nie da się edytować — jest zapisany w środku osoby.";

fn id_arg(args: &Value) -> Result<String, ApiError> {
    let id = args.get("id").and_then(Value::as_str).ok_or_else(|| ApiError::bad_args("id"))?;
    if id.contains('#') {
        return Err(ApiError::new("bad_args", INSIDE_PERSON));
    }
    Ok(id.to_string())
}

/// The shown file and the original of a photo record, as written in the archive.
fn files_of(s: &mut Session, id: &str) -> Result<(Option<String>, Option<String>), ApiError> {
    let media = s.derived().view.media.get(id).ok_or_else(|| ApiError::new("no_record", format!("Nie ma rekordu {id}.")))?;
    Ok((media.file.clone(), media.original.clone()))
}

/// The ids of the people whose link to the object has a `CROP` (it was cut from the old pixels).
fn cropped_links(s: &Session, id: &str) -> Vec<String> {
    s.archive
        .doc
        .records
        .iter()
        .filter(|r| r.tag == "INDI")
        .filter(|r| r.children.iter().any(|c| c.tag == "OBJE" && c.pointer() == Some(id) && c.child("CROP").is_some()))
        .filter_map(|r| r.xref.clone())
        .collect()
}

/// Takes the `CROP` off every person's link to the object; how many links had one.
fn drop_crops(changes: &mut Changes, id: &str, people: &[String]) -> Result<usize, ApiError> {
    let mut dropped = 0;
    for person in people {
        for link in changes.get(person)?.children.iter_mut().filter(|c| c.tag == "OBJE" && c.pointer() == Some(id)) {
            let before = link.children.len();
            link.children.retain(|c| c.tag != "CROP");
            dropped += usize::from(link.children.len() < before);
        }
    }
    Ok(dropped)
}

fn first_file(object: &Node, id: &str) -> Result<usize, ApiError> {
    object.children.iter().position(|c| c.tag == "FILE").ok_or_else(|| ApiError::new("no_file", format!("Rekord {id} nie ma pliku.")))
}

fn set_version(file: &mut Node, version: Option<&str>) {
    file.children.retain(|c| c.tag != "_HLM_VERSION");
    if let Some(version) = version {
        file.children.push(Node::with_value("_HLM_VERSION", version));
    }
}

/// The picture after the quarter turns, the straightening and the crop, in that order (the order the editor previews).
fn apply(img: &DynamicImage, quarter: u8, angle: f64, crop: Option<&Crop>) -> DynamicImage {
    let mut img = rotate_quarter(img, quarter);
    img = straighten(&img, angle);
    match crop {
        Some(c) => crop_fraction(&img, c.x, c.y, c.w, c.h),
        None => img,
    }
}

fn encode(img: &DynamicImage, jpeg: bool) -> Result<Vec<u8>, ApiError> {
    let mut bytes = Vec::new();
    let done = if jpeg {
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY);
        img.to_rgb8().write_with_encoder(encoder)
    } else {
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
    };
    done.map_err(|e| ApiError::new("io", format!("Nie udało się zapisać zdjęcia: {e}")))?;
    Ok(bytes)
}

/// Writes the bytes into the archive under `name` (or the first free name like it) in `dir`, via a fresh temp folder
/// and the copy that never replaces a file. Returns the path relative to the archive and the file made.
fn write_new(s: &Session, dir: &str, name: &str, bytes: &[u8]) -> Result<(String, std::path::PathBuf), ApiError> {
    let temp = std::env::temp_dir().join(format!("heirloom-edit-{}", crate::gedwrite::new_uid()));
    let io = |e: std::io::Error| ApiError::new("io", format!("Nie można zapisać pliku {name}: {e}"));
    std::fs::create_dir_all(&temp).map_err(io)?;
    let result = std::fs::write(temp.join(name), bytes).map_err(io).and_then(|()| crate::media_edit::copy_new_into_archive(&s.layers(), dir, &temp.join(name)));
    let _ = std::fs::remove_dir_all(&temp);
    let (relative, made) = result?;
    // The temp folder is outside the archive, so the copy always makes a file.
    made.map(|file| (relative, file)).ok_or_else(|| ApiError::new("io", format!("Nie można zapisać pliku {name}.")))
}

/// The new file becomes the first `FILE` (a copy of the shown one, so its caption and kind stay); the shown one follows
/// it as the original (the first time) or an earlier version, without the caption. Returns how many links lost a crop.
fn record_new_version(changes: &mut Changes, id: &str, relative: &str, people: &[String]) -> Result<usize, ApiError> {
    let object = changes.get(id)?;
    let first = first_file(object, id)?;
    let has_original = object.children_tagged("FILE").any(|f| f.child_value("_HLM_VERSION").map(str::trim) == Some("original"));
    let mut shown = object.children[first].clone();
    shown.value = Some(file_uri(relative));
    set_version(&mut shown, None);
    match shown.child_mut("FORM") {
        Some(form) => form.value = Some(media_type(relative).to_string()),
        None => shown.children.insert(0, Node::with_value("FORM", media_type(relative))),
    }
    let old = &mut object.children[first];
    old.children.retain(|c| c.tag != "TITL");
    set_version(old, Some(if has_original { "earlier" } else { "original" }));
    object.children.insert(first, shown);
    drop_crops(changes, id, people)
}

fn edit_photo(s: &mut Session, args: &Value) -> ApiResult {
    let id = id_arg(args)?;
    let quarter = match args.get("rotate").map(Value::as_i64) {
        None | Some(Some(0)) => 0,
        Some(Some(90)) => 1,
        Some(Some(180)) => 2,
        Some(Some(270)) => 3,
        _ => return Err(ApiError::bad_args("rotate")),
    };
    let angle = match args.get("angle") {
        None | Some(Value::Null) => 0.0,
        Some(v) => v.as_f64().filter(|a| a.abs() <= MAX_ANGLE).ok_or_else(|| ApiError::bad_args("angle"))?,
    };
    let crop = crop_arg(args)?;
    let (file, original) = files_of(s, &id)?;
    let file = file.ok_or_else(|| ApiError::new("not_found", "Nie ma tego pliku na dysku."))?;
    let path = s.file_roots().resolve(&file);
    if !path.is_file() {
        return Err(ApiError::new("not_found", "Nie ma tego pliku na dysku."));
    }
    if quarter == 0 && angle.abs() < MIN_ANGLE && crop.is_none() {
        return Err(ApiError::new("no_change", "Nic nie zmieniono."));
    }
    let cannot = || ApiError::new("unsupported", "Tego pliku nie da się edytować w Heirloom.");
    let jpeg = ImageReader::open(&path).ok().and_then(|r| r.with_guessed_format().ok()).and_then(|r| r.format()) == Some(ImageFormat::Jpeg);
    let image = crate::media::load_image(&path).ok_or_else(cannot)?;
    let edited = apply(&image, quarter, angle, crop.as_ref());

    // The new name: the original's stem (the photo as it came) + „ (edytowane)”.
    let base = original.as_deref().unwrap_or(&file);
    let name = base.rsplit(['/', '\\']).next().unwrap_or(base);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let name = format!("{stem} (edytowane).{}", if jpeg { "jpg" } else { "png" });
    let dir = crate::layout::dir_for_object(s, &id);
    let (relative, made) = write_new(s, &dir, &name, &encode(&edited, jpeg)?)?;

    let people = cropped_links(s, &id);
    let mut changes = Changes::new(&s.archive.doc);
    let dropped = record_new_version(&mut changes, &id, &relative, &people).map(|dropped| (dropped, changes.into_edits()));
    let dropped = match dropped.and_then(|(dropped, edits)| commit(s, edits).map(|()| dropped)) {
        Ok(dropped) => dropped,
        Err(e) => {
            // Nothing refers to the new file: take it away again.
            let _ = std::fs::remove_file(&made);
            return Err(e);
        }
    };
    Ok(json!({ "path": relative, "droppedCrops": dropped }))
}

fn commit(s: &mut Session, edits: Vec<heirloom_core::Edit>) -> Result<(), ApiError> {
    if !edits.is_empty() {
        s.archive.apply_all(edits)?;
        s.changed();
    }
    Ok(())
}

fn restore_original(s: &mut Session, args: &Value) -> ApiResult {
    let id = id_arg(args)?;
    let (_, original) = files_of(s, &id)?;
    let original = original.ok_or_else(|| ApiError::new("not_edited", "To zdjęcie nie było edytowane."))?;
    if !s.file_roots().resolve(&original).is_file() {
        return Err(ApiError::new("not_found", format!("Nie ma pliku oryginału: {original}. Sprawdź „Brakujące pliki”.")));
    }
    let people = cropped_links(s, &id);
    let mut changes = Changes::new(&s.archive.doc);
    let object = changes.get(&id)?;
    let first = first_file(object, &id)?;
    let at = object
        .children
        .iter()
        .position(|c| c.tag == "FILE" && c.child_value("_HLM_VERSION").map(str::trim) == Some("original"))
        .ok_or_else(|| ApiError::new("not_edited", "To zdjęcie nie było edytowane."))?;
    if at == first {
        // The first file is the original itself: only the marker goes.
        set_version(&mut object.children[first], None);
    } else {
        let mut restored = object.children.remove(at);
        set_version(&mut restored, None);
        // The caption stays with the shown picture: it moves to the original.
        let shown = &mut object.children[first];
        if let Some(pos) = shown.children.iter().position(|c| c.tag == "TITL") {
            let title = shown.children.remove(pos);
            restored.children.retain(|c| c.tag != "TITL");
            restored.children.push(title);
        }
        set_version(shown, Some("earlier"));
        object.children.insert(first, restored);
    }
    let dropped = drop_crops(&mut changes, &id, &people)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "path": original, "droppedCrops": dropped }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Api;
    use image::{Rgb, RgbImage};
    use std::path::Path;

    // ---------- pixels ----------

    const RED: Rgb<u8> = Rgb([220, 30, 30]);
    const BLUE: Rgb<u8> = Rgb([30, 30, 220]);

    /// Left half red, right half blue.
    fn halves(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, _| if x < width / 2 { RED } else { BLUE }))
    }

    #[test]
    fn a_straightening_of_90_degrees_is_a_quarter_turn() {
        // A square with one coloured corner.
        let mut img = RgbImage::from_pixel(24, 24, Rgb([10, 200, 10]));
        for y in 0..6 {
            for x in 0..6 {
                img.put_pixel(x, y, RED);
            }
        }
        let img = DynamicImage::ImageRgb8(img);
        let turned = straighten(&img, 90.0).to_rgb8();
        let quarter = rotate_quarter(&img, 1).to_rgb8();
        assert_eq!(turned.dimensions(), quarter.dimensions());
        for (a, b) in turned.pixels().zip(quarter.pixels()) {
            assert!(a.0.iter().zip(b.0).all(|(&x, y)| x.abs_diff(y) <= 1), "{a:?} vs {b:?}");
        }
        // Clockwise: the top-left corner went to the top-right.
        assert_eq!(*quarter.get_pixel(23, 0), RED);
        assert_eq!(*rotate_quarter(&img, 3).to_rgb8().get_pixel(0, 23), RED);
        assert_eq!(rotate_quarter(&img, 4).to_rgb8(), img.to_rgb8());
    }

    #[test]
    fn straightening_keeps_the_aspect_and_the_largest_rectangle_without_empty_corners() {
        let img = halves(400, 300);
        assert_eq!(straighten(&img, 10.0).dimensions(), (329, 247));
        assert_eq!(straighten(&img, -10.0).dimensions(), (329, 247));
        assert_eq!(straighten(&img, 15.0).dimensions(), (305, 229));
        assert!((straighten_scale(400.0, 300.0, 10.0) - 0.822_139_454_8).abs() < 1e-9);
        // Nothing to straighten.
        assert_eq!(straighten(&img, 0.005).dimensions(), (400, 300));
        // Positive degrees turn clockwise: the red/blue edge leans so that its top end moves right.
        let turned = straighten(&halves(200, 100), 10.0).to_rgb8();
        let edge = |y: u32| (0..turned.width()).find(|&x| turned.get_pixel(x, y).0[2] > 120).unwrap();
        assert!(edge(0) > turned.width() / 2 && edge(turned.height() - 1) < turned.width() / 2, "{} {}", edge(0), edge(turned.height() - 1));
    }

    #[test]
    fn a_crop_is_taken_in_fractions_and_kept_inside_the_image() {
        let img = halves(40, 20);
        let right = crop_fraction(&img, 0.5, 0.0, 0.5, 1.0);
        assert_eq!(right.dimensions(), (20, 20));
        assert!(right.to_rgb8().pixels().all(|p| *p == BLUE));
        let middle = crop_fraction(&img, 0.25, 0.25, 0.5, 0.5);
        assert_eq!(middle.dimensions(), (20, 10));
        // Too big or outside: cut at the edges; never empty.
        assert_eq!(crop_fraction(&img, -0.5, -0.5, 3.0, 3.0).dimensions(), (40, 20));
        assert_eq!(crop_fraction(&img, 0.99, 0.99, 0.5, 0.5).dimensions(), (1, 1));
        assert_eq!(crop_fraction(&img, 0.3, 0.3, 0.0, 0.0).dimensions(), (1, 1));
        assert_eq!(crop_fraction(&img, 1.5, 0.0, 0.2, 1.0).dimensions(), (1, 20));
    }

    // ---------- the commands ----------

    const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Józef /Kowalski/\n1 SEX M\n1 OBJE @O1@\n2 _HLM_PROFILE Y\n2 CROP\n3 TOP 2\n3 LEFT 3\n3 HEIGHT 10\n3 WIDTH 10\n\
0 @I2@ INDI\n1 NAME Anna /Nowak/\n1 SEX F\n1 OBJE @O1@\n\
0 @O1@ OBJE\n1 FILE media/jozef.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n2 TITL Józef w mundurze\n\
0 @O2@ OBJE\n1 FILE media/brak.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n\
0 @O3@ OBJE\n1 FILE media/tekst.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n\
0 @O4@ OBJE\n1 FILE media/stara.bmp\n2 FORM image/bmp\n3 MEDI PHOTO\n2 TITL Stare zdjęcie\n\
0 TRLR\n";

    /// 40×30 pictures in the given files (the format follows the extension).
    fn put_photos(root: &Path, files: &[&str]) {
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            halves(40, 30).to_rgb8().save(&path).unwrap();
        }
        std::fs::write(root.join("media/tekst.jpg"), "to nie jest zdjęcie").unwrap();
    }

    /// An archive worked on in place (`files` exist in it), or `None` to leave the work mode for the test to choose.
    fn open(ged: &str, files: &[&str], mode: Option<&str>) -> (tempfile::TempDir, Api) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(root.join("rodzina.ged"), ged).unwrap();
        put_photos(&root, files);
        let mut api = Api::new(Some(dir.path().join("config")), None);
        api.call("archive.open", json!({ "path": root.to_str().unwrap() })).unwrap();
        if let Some(mode) = mode {
            api.call("archive.setWorkMode", json!({ "mode": mode })).unwrap();
        }
        (dir, api)
    }

    fn in_place() -> (tempfile::TempDir, Api) {
        open(GED, &["media/jozef.jpg", "media/stara.bmp"], Some("inPlace"))
    }

    fn record(api: &Api, xref: &str) -> String {
        heirloom_core::gedcom::line::write(&[api.session.as_ref().unwrap().archive.doc.record(xref).unwrap().clone()], heirloom_core::gedcom::line::Eol::Lf, true)
    }

    fn depth(api: &Api) -> usize {
        api.session.as_ref().unwrap().archive.undo_depth()
    }

    /// The FILE lines of the record: value, and the marker and caption under it.
    fn versions(api: &Api, xref: &str) -> Vec<(String, Option<String>, Option<String>)> {
        let session = api.session.as_ref().unwrap();
        let object = session.archive.doc.record(xref).unwrap();
        object
            .children_tagged("FILE")
            .map(|f| (f.value.clone().unwrap(), f.child_value("_HLM_VERSION").map(str::to_string), f.child_value("TITL").map(str::to_string)))
            .collect()
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    fn size(path: &Path) -> (u32, u32) {
        image::open(path).unwrap().dimensions()
    }

    fn version(file: &str, marker: Option<&str>, title: Option<&str>) -> (String, Option<String>, Option<String>) {
        (file.to_string(), marker.map(str::to_string), title.map(str::to_string))
    }

    #[test]
    fn an_edit_is_a_new_file_and_the_original_is_kept_and_restorable() {
        let (dir, mut api) = in_place();
        let media = dir.path().join("a/media");
        let original_bytes = std::fs::read(media.join("jozef.jpg")).unwrap();
        let before = record(&api, "@O1@");
        let depth0 = depth(&api);

        let done = api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 90 })).unwrap();
        assert_eq!(done["path"], "media/jozef (edytowane).jpg");
        assert_eq!(size(&media.join("jozef (edytowane).jpg")), (30, 40));
        assert_eq!(std::fs::read(media.join("jozef.jpg")).unwrap(), original_bytes, "the original is not touched");
        // The new first FILE keeps the caption and the kind; the old one lost its caption and says what it is.
        assert_eq!(versions(&api, "@O1@"), [version("media/jozef%20(edytowane).jpg", None, Some("Józef w mundurze")), version("media/jozef.jpg", Some("original"), None)]);
        let text = record(&api, "@O1@");
        assert!(text.contains("2 FORM image/jpeg\n3 MEDI PHOTO"), "{text}");
        assert_eq!(depth(&api), depth0 + 1, "one undo step");
        let got = api.call("media.get", json!({ "id": "@O1@" })).unwrap();
        assert_eq!((got["edited"].clone(), got["original"].clone(), got["path"].clone()), (json!(true), json!("media/jozef.jpg"), json!("media/jozef%20(edytowane).jpg".replace("%20", " "))));

        // Again: the new name comes from the original's, and the earlier edit is kept.
        let again = api.call("media.editPhoto", json!({ "id": "@O1@", "angle": 4.5, "crop": { "x": 0.25, "y": 0.0, "w": 0.5, "h": 1.0 } })).unwrap();
        assert_eq!(again["path"], "media/jozef (edytowane) (2).jpg");
        assert_eq!(
            versions(&api, "@O1@"),
            [
                version("media/jozef%20(edytowane)%20(2).jpg", None, Some("Józef w mundurze")),
                version("media/jozef%20(edytowane).jpg", Some("earlier"), None),
                version("media/jozef.jpg", Some("original"), None),
            ]
        );
        // The turned 30×40 picture, straightened by 4.5° (27×36), then the middle half of its width (edges at 6.75 and 20.25).
        assert_eq!(size(&media.join("jozef (edytowane) (2).jpg")), (13, 36));

        // The original comes back first, with the caption; the shown one is kept as an earlier version.
        let restored = api.call("media.restoreOriginal", json!({ "id": "@O1@" })).unwrap();
        assert_eq!((restored["path"].clone(), restored["droppedCrops"].clone()), (json!("media/jozef.jpg"), json!(0)));
        assert_eq!(
            versions(&api, "@O1@"),
            [
                version("media/jozef.jpg", None, Some("Józef w mundurze")),
                version("media/jozef%20(edytowane)%20(2).jpg", Some("earlier"), None),
                version("media/jozef%20(edytowane).jpg", Some("earlier"), None),
            ]
        );
        let got = api.call("media.get", json!({ "id": "@O1@" })).unwrap();
        assert_eq!((got["edited"].clone(), got["path"].clone()), (json!(false), json!("media/jozef.jpg")));
        assert_eq!(
            api.call("media.restoreOriginal", json!({ "id": "@O1@" })).unwrap_err().message,
            "To zdjęcie nie było edytowane."
        );

        // Every file is still referenced (versions count), so the scan finds nothing unknown.
        let scan = api.call("media.scan", json!({})).unwrap();
        // (The one missing file is @O2@'s, which has none on disk.)
        assert_eq!((scan["counts"]["unknown"].clone(), scan["counts"]["missing"].clone()), (json!(0), json!(1)), "{scan}");
        assert_eq!(names(&media), ["jozef (edytowane) (2).jpg", "jozef (edytowane).jpg", "jozef.jpg", "stara.bmp", "tekst.jpg"]);

        // Undo walks back: restore, the second edit, the first.
        for _ in 0..3 {
            api.call("archive.undo", json!({})).unwrap();
        }
        assert_eq!(record(&api, "@O1@"), before);
        assert_eq!(depth(&api), depth0);
    }

    #[test]
    fn the_versions_are_saved_and_read_back() {
        let (dir, mut api) = in_place();
        api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 90 })).unwrap();
        api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 90 })).unwrap();
        api.call("archive.save", json!({ "author": "Ewa", "allowForeign": true })).unwrap();
        let saved = std::fs::read_to_string(dir.path().join("a/rodzina.ged")).unwrap();
        assert!(saved.contains("TAG _HLM_VERSION"), "{saved}");
        assert!(saved.contains("2 _HLM_VERSION original") && saved.contains("2 _HLM_VERSION earlier"), "{saved}");
        let feed = api.call("history.feed", json!({})).unwrap().to_string();
        assert!(feed.contains("Edytowano plik") && feed.contains("jozef (edytowane) (2).jpg"), "{feed}");

        // Opened again, the photo is still an edited one with its original.
        api.call("archive.close", json!({})).unwrap();
        api.call("archive.open", json!({ "path": dir.path().join("a").to_str().unwrap() })).unwrap();
        let got = api.call("media.get", json!({ "id": "@O1@" })).unwrap();
        assert_eq!((got["edited"].clone(), got["original"].clone(), got["path"].clone()), (json!(true), json!("media/jozef.jpg"), json!("media/jozef (edytowane) (2).jpg")));
        assert!(api.call("media.restoreOriginal", json!({ "id": "@O1@" })).is_ok());
    }

    #[test]
    fn a_picture_that_is_not_a_jpeg_is_saved_as_png() {
        let (dir, mut api) = in_place();
        let done = api.call("media.editPhoto", json!({ "id": "@O4@", "crop": { "x": 0.0, "y": 0.0, "w": 0.5, "h": 0.5 } })).unwrap();
        assert_eq!(done["path"], "media/stara (edytowane).png");
        let file = dir.path().join("a/media/stara (edytowane).png");
        assert_eq!(size(&file), (20, 15));
        assert!(std::fs::read(&file).unwrap().starts_with(b"\x89PNG"));
        let text = record(&api, "@O4@");
        assert!(text.contains("1 FILE media/stara%20(edytowane).png\n2 FORM image/png\n3 MEDI PHOTO\n2 TITL Stare zdjęcie"), "{text}");
        assert!(text.contains("1 FILE media/stara.bmp\n2 FORM image/bmp\n3 MEDI PHOTO\n2 _HLM_VERSION original"), "{text}");
    }

    #[test]
    fn a_profile_crop_is_dropped_by_an_edit_and_by_a_restore() {
        let (_dir, mut api) = in_place();
        assert!(record(&api, "@I1@").contains("2 CROP"));
        let done = api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 180 })).unwrap();
        assert_eq!(done["droppedCrops"], 1, "only Józef's link had one");
        let jozef = record(&api, "@I1@");
        assert!(!jozef.contains("CROP") && jozef.contains("2 _HLM_PROFILE Y"), "{jozef}");
        assert!(record(&api, "@I2@").contains("1 OBJE @O1@"));

        // A crop set on the new picture goes when the original comes back.
        api.call("media.setProfile", json!({ "id": "@O1@", "person": "@I1@" })).unwrap();
        let mut json_crop = json!({ "id": "@O1@", "person": "@I1@" });
        json_crop["crop"] = json!({ "top": 1, "left": 1, "height": 5, "width": 5 });
        api.call("media.setProfile", json_crop).unwrap();
        assert!(record(&api, "@I1@").contains("CROP"));
        let restored = api.call("media.restoreOriginal", json!({ "id": "@O1@" })).unwrap();
        assert_eq!(restored["droppedCrops"], 1);
        assert!(!record(&api, "@I1@").contains("CROP"));
        // The edit and the dropped crop are one undo step.
        let depth_now = depth(&api);
        api.call("archive.undo", json!({})).unwrap();
        assert_eq!(depth(&api), depth_now - 1);
        assert!(record(&api, "@I1@").contains("CROP"));
    }

    #[test]
    fn wrong_requests_are_refused_and_change_nothing() {
        let (dir, mut api) = in_place();
        let depth0 = depth(&api);
        let before = names(&dir.path().join("a/media"));
        let mut refuse = |args: Value| api.call("media.editPhoto", args).unwrap_err();

        assert_eq!(refuse(json!({ "id": "@I1@#0", "rotate": 90 })).message, "Tego pliku nie da się edytować — jest zapisany w środku osoby.");
        assert_eq!(refuse(json!({ "id": "@O2@", "rotate": 90 })).message, "Nie ma tego pliku na dysku.");
        assert_eq!(refuse(json!({ "id": "@O3@", "rotate": 90 })).message, "Tego pliku nie da się edytować w Heirloom.");
        assert_eq!(refuse(json!({ "id": "@O1@" })).message, "Nic nie zmieniono.");
        assert_eq!(refuse(json!({ "id": "@O1@", "rotate": 0, "angle": 0.004, "crop": null })).message, "Nic nie zmieniono.");
        assert_eq!(refuse(json!({ "id": "@O1@", "crop": { "x": 0, "y": 0, "w": 1, "h": 1 } })).message, "Nic nie zmieniono.", "the whole picture is no crop");
        assert_eq!(refuse(json!({ "id": "@O1@", "angle": 15.5 })).code, "bad_args");
        assert_eq!(refuse(json!({ "id": "@O1@", "angle": "5" })).code, "bad_args");
        assert_eq!(refuse(json!({ "id": "@O1@", "rotate": 45 })).code, "bad_args");
        assert_eq!(refuse(json!({ "id": "@O1@", "crop": { "x": 0.1 } })).code, "bad_args");
        assert_eq!(refuse(json!({ "rotate": 90 })).code, "bad_args");
        assert_eq!(refuse(json!({ "id": "@O9@", "rotate": 90 })).code, "no_record");
        assert_eq!(api.call("media.restoreOriginal", json!({ "id": "@O1@" })).unwrap_err().message, "To zdjęcie nie było edytowane.");
        assert_eq!(api.call("media.restoreOriginal", json!({ "id": "@I1@#0" })).unwrap_err().code, "bad_args");
        assert_eq!(depth(&api), depth0);
        assert_eq!(names(&dir.path().join("a/media")), before);

        // The original's file is gone: the restore says which.
        api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 270 })).unwrap();
        std::fs::remove_file(dir.path().join("a/media/jozef.jpg")).unwrap();
        let error = api.call("media.restoreOriginal", json!({ "id": "@O1@" })).unwrap_err();
        assert_eq!(error.message, "Nie ma pliku oryginału: media/jozef.jpg. Sprawdź „Brakujące pliki”.");
        assert_eq!(depth(&api), depth0 + 1);
    }

    #[test]
    fn in_nie_ruszaj_oryginalu_the_new_file_goes_to_the_changes_folder() {
        let (dir, mut api) = open(GED, &["media/jozef.jpg", "media/stara.bmp"], None);
        let status = api.call("archive.setWorkMode", json!({ "mode": "overlay" })).unwrap();
        let changes = std::path::PathBuf::from(status["overlay"]["folder"].as_str().unwrap());
        let original_media = dir.path().join("a/media");
        let original_before: Vec<(String, Vec<u8>)> = names(&original_media).into_iter().map(|n| (n.clone(), std::fs::read(original_media.join(&n)).unwrap())).collect();

        let done = api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 90 })).unwrap();
        assert_eq!(done["path"], "media/jozef (edytowane).jpg");
        assert_eq!(size(&changes.join("media/jozef (edytowane).jpg")), (30, 40));
        let original_after: Vec<(String, Vec<u8>)> = names(&original_media).into_iter().map(|n| (n.clone(), std::fs::read(original_media.join(&n)).unwrap())).collect();
        assert_eq!(original_after, original_before, "nothing new in the original archive");
        // The shown picture is served from the changes folder, the original from the archive.
        let got = api.call("media.get", json!({ "id": "@O1@" })).unwrap();
        assert_eq!((got["layer"].clone(), got["edited"].clone()), (json!("mine"), json!(true)));
        // A second edit takes a name free in both folders.
        let again = api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 90 })).unwrap();
        assert_eq!(again["path"], "media/jozef (edytowane) (2).jpg");
        assert!(changes.join("media/jozef (edytowane) (2).jpg").is_file());
        let restored = api.call("media.restoreOriginal", json!({ "id": "@O1@" })).unwrap();
        assert_eq!(restored["path"], "media/jozef.jpg");
    }

    #[test]
    fn in_a_folder_of_the_person_the_edited_file_lands_in_the_same_folder() {
        let ged = GED.replace("media/jozef.jpg", "media/Kowalski Józef/jozef.jpg");
        let (dir, mut api) = open(&ged, &["media/Kowalski Józef/jozef.jpg", "media/stara.bmp"], Some("inPlace"));
        api.call("media.setLayout", json!({ "layout": "perPerson" })).unwrap();
        let done = api.call("media.editPhoto", json!({ "id": "@O1@", "rotate": 90 })).unwrap();
        assert_eq!(done["path"], "media/Kowalski Józef/jozef (edytowane).jpg");
        assert!(dir.path().join("a/media/Kowalski Józef/jozef (edytowane).jpg").is_file());
        assert!(!dir.path().join("a/media/jozef (edytowane).jpg").exists());
    }
}
