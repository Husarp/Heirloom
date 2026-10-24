//! The crop of a person's profile photo (`CROP` under the person's link to the photo).

use crate::edit::Changes;
use crate::{ApiError, ApiResult, Session};
use heirloom_core::gedcom::Node;
use serde_json::Value;

/// The smallest side of a crop, in pixels.
const MIN_SIDE: u32 = 8;

pub fn call(s: &mut Session, method: &str, args: &Value) -> ApiResult {
    match method {
        "media.setCrop" => set_crop(s, args),
        _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
    }
}

/// A crop given as fractions 0–1 of the upright image (`x`, `y`, `w`, `h`) in pixels: `[left, top, width, height]`.
/// The frame is square (the shorter of the two sides) and lies inside the image; the result is at least 8 px.
/// None when the numbers are not a crop. Import step 4 uses the same conversion.
pub(crate) fn pixels(size: (u32, u32), crop: &Value) -> Option<[u32; 4]> {
    let (width, height) = size;
    let num = |key: &str| crop.get(key).and_then(Value::as_f64).filter(|v| v.is_finite());
    let (x, y, w, h) = (num("x")?, num("y")?, num("w")?, num("h")?);
    if width == 0 || height == 0 || !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (fw, fh) = (f64::from(width), f64::from(height));
    let shorter = width.min(height);
    let side = ((w * fw).min(h * fh).round() as u32).clamp(MIN_SIDE.min(shorter), shorter);
    let left = ((x * fw).round() as u32).min(width - side);
    let top = ((y * fh).round() as u32).min(height - side);
    Some([left, top, side, side])
}

pub(crate) fn crop_node([left, top, width, height]: [u32; 4]) -> Node {
    let mut node = Node::new("CROP");
    for (tag, v) in [("TOP", top), ("LEFT", left), ("HEIGHT", height), ("WIDTH", width)] {
        node.children.push(Node::with_value(tag, &v.to_string()));
    }
    node
}

/// „Kadruj zdjęcie profilowe”: the visible part of a photo on one person's profile; the photo file does not change.
fn set_crop(s: &mut Session, args: &Value) -> ApiResult {
    let person = args.get("person").and_then(Value::as_str).ok_or_else(|| ApiError::bad_args("person"))?;
    let id = args.get("id").and_then(Value::as_str).ok_or_else(|| ApiError::bad_args("id"))?;
    let crop = args.get("crop").ok_or_else(|| ApiError::bad_args("crop"))?;
    let not_there = || ApiError::new("not_found", "To zdjęcie nie jest na profilu tej osoby.");

    let linked = s.archive.doc.record(person).is_some_and(|r| r.tag == "INDI" && r.children.iter().any(|c| c.tag == "OBJE" && c.pointer() == Some(id)));
    if !linked {
        return Err(not_there());
    }
    let node = if crop.is_null() {
        None
    } else {
        let roots = s.file_roots();
        let file = s.derived().view.media.get(id).and_then(|m| m.file.clone());
        let path = file.map(|f| roots.resolve(&f)).filter(|p| p.is_file());
        let size = path.as_deref().and_then(crate::media::upright_size).ok_or_else(|| {
            ApiError::new("no_file", "Nie można odczytać zdjęcia, więc nie da się wybrać kadru. Sprawdź, czy plik jest na swoim miejscu.")
        })?;
        Some(crop_node(pixels(size, crop).ok_or_else(|| ApiError::bad_args("crop"))?))
    };

    let mut changes = Changes::new(&s.archive.doc);
    for link in changes.get(person)?.children.iter_mut().filter(|c| c.tag == "OBJE" && c.pointer() == Some(id)) {
        match (&node, link.children.iter().position(|c| c.tag == "CROP")) {
            (Some(n), Some(at)) => link.children[at] = n.clone(),
            (Some(n), None) => link.children.push(n.clone()),
            (None, Some(at)) => {
                link.children.remove(at);
            }
            (None, None) => {}
        }
    }
    let edits = changes.into_edits();
    if !edits.is_empty() {
        s.archive.apply_all(edits)?;
        s.changed();
    }
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::pixels;
    use crate::Api;
    use serde_json::{Value, json};

    const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 SEX M\n1 OBJE @O1@\n2 _HLM_PROFILE Y\n\
0 @I2@ INDI\n1 NAME Ewa /Kowal/\n\
0 @O1@ OBJE\n1 FILE media/p.png\n2 FORM image/png\n3 MEDI PHOTO\n\
0 @O2@ OBJE\n1 FILE media/gone.png\n2 FORM image/png\n3 MEDI PHOTO\n\
0 TRLR\n";

    fn open() -> (tempfile::TempDir, Api) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(root.join("rodzina.ged"), GED).unwrap();
        image::RgbImage::new(400, 200).save(root.join("media/p.png")).unwrap();
        let mut api = Api::new(Some(dir.path().join("config")), None);
        api.call("archive.open", json!({ "path": root.to_str().unwrap() })).unwrap();
        api.call("archive.setWorkMode", json!({ "mode": "inPlace" })).unwrap();
        (dir, api)
    }

    fn record(api: &mut Api, xref: &str) -> String {
        heirloom_core::gedcom::line::write(&[api.session.as_ref().unwrap().archive.doc.record(xref).unwrap().clone()], heirloom_core::gedcom::line::Eol::Lf, true)
    }

    fn depth(api: &Api) -> usize {
        api.session.as_ref().unwrap().archive.undo_depth()
    }

    #[test]
    fn fractions_become_a_square_of_pixels_inside_the_image() {
        let size = (400, 200);
        assert_eq!(pixels(size, &json!({ "x": 0.25, "y": 0.0, "w": 0.5, "h": 1.0 })), Some([100, 0, 200, 200]));
        // The shorter side wins; a frame pushed past the edge is moved back inside.
        assert_eq!(pixels(size, &json!({ "x": 0.9, "y": 0.5, "w": 0.5, "h": 0.25 })), Some([350, 100, 50, 50]));
        assert_eq!(pixels(size, &json!({ "x": 0.0, "y": 0.0, "w": 0.001, "h": 0.001 })), Some([0, 0, 8, 8]));
        assert_eq!(pixels((6, 400), &json!({ "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0 })), Some([0, 0, 6, 6]));
        for bad in [json!({}), json!({ "x": 0.1, "y": 0.1, "w": 0.0, "h": 0.5 }), json!({ "x": 1.5, "y": 0.0, "w": 0.5, "h": 0.5 }), json!({ "x": 0.1, "y": 0.1, "w": 0.5, "h": "a" })] {
            assert_eq!(pixels(size, &bad), None, "{bad}");
        }
        assert_eq!(pixels((0, 10), &json!({ "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0 })), None);
    }

    #[test]
    fn a_crop_is_set_shown_changed_removed_and_undone() {
        let (_dir, mut api) = open();
        let before = record(&mut api, "@I1@");
        let depth0 = depth(&api);
        let set = json!({ "person": "@I1@", "id": "@O1@", "crop": { "x": 0.25, "y": 0.0, "w": 0.5, "h": 1.0 } });

        api.call("media.setCrop", set.clone()).unwrap();
        let text = record(&mut api, "@I1@");
        assert!(text.contains("2 CROP\n3 TOP 0\n3 LEFT 100\n3 HEIGHT 200\n3 WIDTH 200"), "{text}");
        assert_eq!(depth(&api), depth0 + 1);
        let jan = api.call("person.get", json!({ "id": "@I1@" })).unwrap();
        assert_eq!(jan["portrait"]["thumb"], "crop:100,0,200,200/media/p.png");
        assert_eq!(jan["person"]["photo"], "crop:100,0,200,200/media/p.png");
        let crop = &jan["portrait"]["crop"];
        let near = |k: &str, v: f64| assert!((crop[k].as_f64().unwrap() - v).abs() < 1e-9, "{k}: {crop}");
        (near("x", 0.25), near("y", 0.0), near("w", 0.5), near("h", 1.0));

        // The same value again changes nothing; a new one replaces the old CROP.
        api.call("media.setCrop", set).unwrap();
        assert_eq!(depth(&api), depth0 + 1);
        api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O1@", "crop": { "x": 0.0, "y": 0.0, "w": 0.25, "h": 0.5 } })).unwrap();
        let text = record(&mut api, "@I1@");
        assert_eq!(text.matches("CROP").count(), 1, "{text}");
        assert!(text.contains("3 LEFT 0") && text.contains("3 WIDTH 100"), "{text}");

        api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O1@", "crop": null })).unwrap();
        assert_eq!(record(&mut api, "@I1@").split("1 CHAN").next(), Some(before.as_str()), "only the change stamp is left");
        let jan = api.call("person.get", json!({ "id": "@I1@" })).unwrap();
        assert_eq!((jan["portrait"]["thumb"].as_str(), jan["portrait"]["crop"].is_null()), (Some("media/p.png"), true));
        let depth1 = depth(&api);
        api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O1@", "crop": null })).unwrap();
        assert_eq!(depth(&api), depth1, "nothing to remove");

        // Undo, step by step, down to the start.
        api.call("archive.undo", json!({})).unwrap();
        assert!(record(&mut api, "@I1@").contains("3 WIDTH 100"));
        while depth(&api) > depth0 {
            api.call("archive.undo", json!({})).unwrap();
        }
        assert_eq!(record(&mut api, "@I1@"), before);
    }

    #[test]
    fn a_photo_that_is_not_on_the_profile_or_not_readable_is_refused() {
        let (_dir, mut api) = open();
        let depth0 = depth(&api);
        let crop = json!({ "x": 0.0, "y": 0.0, "w": 0.5, "h": 0.5 });
        let error = api.call("media.setCrop", json!({ "person": "@I2@", "id": "@O1@", "crop": crop })).unwrap_err();
        assert_eq!(error.message, "To zdjęcie nie jest na profilu tej osoby.");
        assert_eq!(api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O2@", "crop": crop })).unwrap_err().code, "not_found");
        assert_eq!(api.call("media.setCrop", json!({ "person": "@I9@", "id": "@O1@", "crop": crop })).unwrap_err().code, "not_found");
        assert_eq!(api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O1@" })).unwrap_err().code, "bad_args");
        assert_eq!(api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O1@", "crop": { "x": 0 } })).unwrap_err().code, "bad_args");
        assert_eq!(depth(&api), depth0);

        // Linked but the file is gone: a Polish refusal (removing a crop needs no file).
        let ged = std::fs::read_to_string(_dir.path().join("a/rodzina.ged")).unwrap().replace("1 OBJE @O1@\n2 _HLM_PROFILE Y", "1 OBJE @O2@\n2 _HLM_PROFILE Y");
        std::fs::write(_dir.path().join("a/rodzina.ged"), ged).unwrap();
        api.call("archive.open", json!({ "path": _dir.path().join("a").to_str().unwrap() })).unwrap();
        api.call("archive.setWorkMode", json!({ "mode": "inPlace" })).unwrap();
        let error = api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O2@", "crop": crop })).unwrap_err();
        assert_eq!(error.code, "no_file");
        assert!(api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O2@", "crop": Value::Null })).is_ok());
    }

    #[test]
    fn a_saved_crop_is_named_in_the_history() {
        let (dir, mut api) = open();
        api.call("media.setCrop", json!({ "person": "@I1@", "id": "@O1@", "crop": { "x": 0.25, "y": 0.0, "w": 0.5, "h": 1.0 } })).unwrap();
        api.call("archive.save", json!({ "author": "Ewa", "allowForeign": true })).unwrap();
        let saved = std::fs::read_to_string(dir.path().join("a/rodzina.ged")).unwrap();
        assert!(saved.contains("3 LEFT 100"), "{saved}");
        let jan = api.call("person.get", json!({ "id": "@I1@" })).unwrap();
        let history = jan["history"].to_string();
        assert!(history.contains("Kadr zdjęcia profilowego") && history.contains("100, 0, 200×200"), "{history}");
    }
}
