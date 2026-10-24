//! „Do uzupełnienia”: what still needs finishing (unfinished people, photos and documents without a caption) and the
//! three small commands that mark a person unfinished or leave something out of the list.

use crate::derive::Derived;
use crate::edit::Changes;
use crate::gedwrite::insert_after;
use crate::lists::{media_json, media_people, undescribed};
use crate::media_edit::FileRoots;
use crate::people::years_range;
use crate::{ApiError, ApiResult, Session};
use heirloom_core::fold::fold;
use heirloom_core::gedcom::Node;
use heirloom_core::gedcom::view::{MediaKind, TextKind};
use serde_json::{Value, json};

/// How much of the first research note shows under an unfinished person.
const HINT_CHARS: usize = 160;

/// The person's first research note as one line of plain text, cut with „…”.
fn hint(d: &Derived, i: usize) -> Option<String> {
    let note = d.view.person_texts(&d.view.people[i]).find(|t| t.kind == TextKind::Note)?;
    let plain = crate::text::plain(&note.body);
    let line = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return None;
    }
    let chars: Vec<char> = line.chars().collect();
    Some(if chars.len() > HINT_CHARS { format!("{}…", chars[..HINT_CHARS].iter().collect::<String>().trim_end()) } else { line })
}

fn person_row(d: &Derived, i: usize) -> Value {
    let mut v = d.summary(i);
    v["years"] = json!(years_range(d, i));
    v["linked"] = json!(d.info[i].linked);
    v["batch"] = json!(d.view.people[i].batch);
    v["hint"] = json!(hint(d, i));
    v
}

pub fn list(d: &Derived, roots: &FileRoots) -> Value {
    let mut unfinished: Vec<usize> = (0..d.info.len()).filter(|&i| d.info[i].unfinished).collect();
    unfinished.sort_by_cached_key(|&i| fold(&d.info[i].name));
    let (skipped, open): (Vec<usize>, Vec<usize>) = unfinished.into_iter().partition(|&i| d.view.people[i].ignored);
    let rows = |list: &[usize]| list.iter().map(|&i| person_row(d, i)).collect::<Vec<_>>();

    let people_of = media_people(d);
    let (mut photos, mut documents, mut ignored_photos, mut ignored_documents) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for id in &d.view.media_order {
        let Some(m) = d.view.media.get(id).filter(|m| undescribed(m)) else { continue };
        let Some(json) = media_json(d, id, roots, &people_of) else { continue };
        match (m.kind == MediaKind::Photo, m.ignored) {
            (true, false) => photos.push(json),
            (true, true) => ignored_photos.push(json),
            (false, false) => documents.push(json),
            (false, true) => ignored_documents.push(json),
        }
    }
    let counts = json!({
        "people": open.len(),
        "photos": photos.len(),
        "documents": documents.len(),
        "ignored": skipped.len() + ignored_photos.len() + ignored_documents.len(),
    });
    json!({
        "people": rows(&open),
        "photos": photos,
        "documents": documents,
        "ignored": { "people": rows(&skipped), "photos": ignored_photos, "documents": ignored_documents },
        "counts": counts,
    })
}

pub fn call(s: &mut Session, method: &str, args: &Value) -> ApiResult {
    match method {
        "person.setTodo" => set_todo(s, args),
        "person.setIgnored" => set_ignored(s, "INDI", args),
        "media.setIgnored" => {
            if args.get("id").and_then(Value::as_str).is_some_and(|id| id.contains('#')) {
                return Err(ApiError::new("bad_args", "Tego pliku nie da się pominąć — jest zapisany w środku osoby."));
            }
            set_ignored(s, "OBJE", args)
        }
        _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
    }
}

fn id_arg(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ApiError::bad_args(key))
}

fn flag_arg(args: &Value, key: &str) -> Result<bool, ApiError> {
    args.get(key).and_then(Value::as_bool).ok_or_else(|| ApiError::bad_args(key))
}

/// The record `id` must exist and be of the given kind (a person command on a photo is a mistake of the caller).
fn check_record(s: &Session, id: &str, tag: &str) -> Result<(), ApiError> {
    match s.archive.doc.record(id) {
        Some(r) if r.tag == tag => Ok(()),
        _ => Err(ApiError::new("no_record", format!("Nie ma rekordu {id}."))),
    }
}

/// `1 <tag> Y` on the record, or none. Left as it is when it already says so, so a repeated command changes nothing.
fn set_flag(record: &mut Node, tag: &str, on: bool) {
    let is_yes = |c: &Node| c.value.as_deref().is_some_and(|v| v.trim().eq_ignore_ascii_case("Y"));
    let mut marks = record.children.iter().filter(|c| c.tag == tag);
    if on && marks.next().is_some_and(is_yes) && marks.next().is_none() {
        return;
    }
    record.children.retain(|c| c.tag != tag);
    if on {
        let after: &[&str] = if record.tag == "INDI" { &["NAME", "SEX"] } else { &["FILE"] };
        insert_after(record, after, Node::with_value(tag, "Y"));
    }
}

fn apply(s: &mut Session, id: &str, edit: impl FnOnce(&mut Node)) -> ApiResult {
    let mut changes = Changes::new(&s.archive.doc);
    edit(changes.get(id)?);
    let edits = changes.into_edits();
    if !edits.is_empty() {
        s.archive.apply_all(edits)?;
        s.changed();
    }
    Ok(Value::Null)
}

/// Marked unfinished: a person who is not done yet. Marking removes „Pomiń”; a finished one loses both.
fn set_todo(s: &mut Session, args: &Value) -> ApiResult {
    let id = id_arg(args, "id")?;
    let on = flag_arg(args, "todo")?;
    check_record(s, &id, "INDI")?;
    apply(s, &id, |r| {
        set_flag(r, "_HLM_IGNORE", false);
        set_flag(r, "_HLM_TODO", on);
    })
}

fn set_ignored(s: &mut Session, tag: &str, args: &Value) -> ApiResult {
    let id = id_arg(args, "id")?;
    let on = flag_arg(args, "ignored")?;
    check_record(s, &id, tag)?;
    apply(s, &id, |r| set_flag(r, "_HLM_IGNORE", on))
}

#[cfg(test)]
mod tests {
    use crate::Api;
    use serde_json::{Value, json};

    const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 SEX M\n1 FAMS @F1@\n1 _HLM_TODO Y\n1 _HLM_BATCH import-1\n1 NOTE Szukać w *księgach* parafii we Wólce, najlepiej przed rokiem 1900, bo później spisy się nie zachowały i trzeba będzie pytać w archiwum diecezjalnym o wypisy z ksiąg chrztów oraz zgonów.\n\
0 @I2@ INDI\n1 NAME Ewa /Kowal/\n1 _HLM_TODO Y\n\
0 @I3@ INDI\n1 NAME Anna /Nowak/\n1 SEX F\n1 FAMS @F1@\n\
0 @I4@ INDI\n1 NAME Adam /Abacki/\n1 _HLM_TODO Y\n1 _HLM_IGNORE Y\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I3@\n\
0 @O1@ OBJE\n1 FILE media/a.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n\
0 @O2@ OBJE\n1 FILE media/IMG_0042.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n2 TITL img_0042\n\
0 @O3@ OBJE\n1 FILE media/c.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n2 TITL Dom w Wólce\n\
0 @O4@ OBJE\n1 FILE media/akt.pdf\n2 FORM application/pdf\n3 MEDI MANUSCRIPT\n\
0 @O5@ OBJE\n1 FILE media/nagranie.mp3\n2 FORM audio/mpeg\n\
0 @O6@ OBJE\n1 FILE media/f.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n1 _HLM_IGNORE Y\n\
0 @O7@ OBJE\n1 FILE media/g.pdf\n2 FORM application/pdf\n3 MEDI MANUSCRIPT\n2 TITL g.pdf\n1 _HLM_IGNORE Y\n\
0 TRLR\n";

    fn open() -> (tempfile::TempDir, Api) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(root.join("rodzina.ged"), GED).unwrap();
        let mut api = Api::new(Some(dir.path().join("config")), None);
        api.call("archive.open", json!({ "path": root.to_str().unwrap() })).unwrap();
        api.call("archive.setWorkMode", json!({ "mode": "inPlace" })).unwrap();
        (dir, api)
    }

    fn ids(v: &Value) -> Vec<&str> {
        v.as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect()
    }

    fn record(api: &mut Api, xref: &str) -> String {
        heirloom_core::gedcom::line::write(&[api.session.as_ref().unwrap().archive.doc.record(xref).unwrap().clone()], heirloom_core::gedcom::line::Eol::Lf, true)
    }

    fn depth(api: &Api) -> usize {
        api.session.as_ref().unwrap().archive.undo_depth()
    }

    #[test]
    fn the_list_has_unfinished_people_and_undescribed_files() {
        let (_dir, mut api) = open();
        let list = api.call("todo.list", json!({})).unwrap();
        // Name order, folded: Ewa Kowal before Jan Nowak; the ignored one is set aside.
        assert_eq!(ids(&list["people"]), ["@I2@", "@I1@"]);
        assert_eq!(ids(&list["ignored"]["people"]), ["@I4@"]);
        let (ewa, jan) = (&list["people"][0], &list["people"][1]);
        assert_eq!((ewa["linked"].clone(), jan["linked"].clone()), (json!(false), json!(true)));
        assert_eq!((ewa["batch"].clone(), jan["batch"].clone()), (Value::Null, json!("import-1")));
        assert!(ewa["hint"].is_null());
        assert_eq!(ewa["name"], "Ewa Kowal");
        let hint = jan["hint"].as_str().unwrap();
        assert!(hint.starts_with("Szukać w księgach parafii we Wólce"), "{hint}");
        assert!(hint.ends_with('…') && hint.chars().count() <= 161, "{hint}");
        // A photo with no caption and one captioned with its file's name; a described one, „inne” and the
        // ignored ones are not in the first lists.
        assert_eq!(ids(&list["photos"]), ["@O1@", "@O2@"]);
        assert_eq!(ids(&list["documents"]), ["@O4@"]);
        assert_eq!(ids(&list["ignored"]["photos"]), ["@O6@"]);
        assert_eq!(ids(&list["ignored"]["documents"]), ["@O7@"]);
        assert_eq!(list["counts"], json!({ "people": 2, "photos": 2, "documents": 1, "ignored": 3 }));
        assert_eq!(list["photos"][0]["undescribed"], true);
    }

    #[test]
    fn marking_and_skipping_are_one_undo_step_each_and_do_nothing_twice() {
        let (_dir, mut api) = open();
        let before = record(&mut api, "@I3@");
        let depth0 = depth(&api);

        api.call("person.setTodo", json!({ "id": "@I3@", "todo": true })).unwrap();
        let marked = record(&mut api, "@I3@");
        assert!(marked.contains("1 _HLM_TODO Y"), "{marked}");
        assert!(marked.find("_HLM_TODO").unwrap() < marked.find("FAMS").unwrap(), "after NAME/SEX: {marked}");
        assert_eq!(depth(&api), depth0 + 1);
        api.call("person.setTodo", json!({ "id": "@I3@", "todo": true })).unwrap();
        assert_eq!(depth(&api), depth0 + 1, "already so: no empty step");

        // Marking takes „Pomiń” away; skipping again puts it back.
        api.call("person.setIgnored", json!({ "id": "@I3@", "ignored": true })).unwrap();
        assert!(record(&mut api, "@I3@").contains("_HLM_IGNORE Y"));
        api.call("person.setTodo", json!({ "id": "@I3@", "todo": true })).unwrap();
        assert!(!record(&mut api, "@I3@").contains("_HLM_IGNORE"));
        // Done: both gone.
        api.call("person.setIgnored", json!({ "id": "@I3@", "ignored": true })).unwrap();
        api.call("person.setTodo", json!({ "id": "@I3@", "todo": false })).unwrap();
        let done = record(&mut api, "@I3@");
        assert!(!done.contains("_HLM_TODO") && !done.contains("_HLM_IGNORE"), "{done}");
        let depth1 = depth(&api);
        api.call("person.setTodo", json!({ "id": "@I3@", "todo": false })).unwrap();
        api.call("person.setIgnored", json!({ "id": "@I3@", "ignored": false })).unwrap();
        assert_eq!(depth(&api), depth1);

        // Undo brings the earlier record back, step by step, down to the start.
        while depth(&api) > depth0 {
            api.call("archive.undo", json!({})).unwrap();
        }
        assert_eq!(record(&mut api, "@I3@"), before);

        // Photos and documents: the same.
        api.call("media.setIgnored", json!({ "id": "@O1@", "ignored": true })).unwrap();
        assert!(record(&mut api, "@O1@").contains("1 _HLM_IGNORE Y"));
        let depth2 = depth(&api);
        api.call("media.setIgnored", json!({ "id": "@O1@", "ignored": true })).unwrap();
        assert_eq!(depth(&api), depth2);
        let list = api.call("todo.list", json!({})).unwrap();
        assert_eq!(ids(&list["photos"]), ["@O2@"]);
        assert_eq!(list["counts"]["ignored"], 4);
        api.call("media.setIgnored", json!({ "id": "@O1@", "ignored": false })).unwrap();
        assert!(!record(&mut api, "@O1@").contains("_HLM_IGNORE"));
        api.call("archive.undo", json!({})).unwrap();
        assert!(record(&mut api, "@O1@").contains("_HLM_IGNORE"));
    }

    #[test]
    fn wrong_ids_and_files_inside_a_person_are_refused() {
        let (_dir, mut api) = open();
        let depth0 = depth(&api);
        let error = api.call("media.setIgnored", json!({ "id": "@I1@#0", "ignored": true })).unwrap_err();
        assert_eq!(error.message, "Tego pliku nie da się pominąć — jest zapisany w środku osoby.");
        assert_eq!(api.call("person.setTodo", json!({ "id": "@I99@", "todo": true })).unwrap_err().code, "no_record");
        // A person command does not work on a photo, nor a photo command on a person.
        assert_eq!(api.call("person.setIgnored", json!({ "id": "@O1@", "ignored": true })).unwrap_err().code, "no_record");
        assert_eq!(api.call("media.setIgnored", json!({ "id": "@I1@", "ignored": true })).unwrap_err().code, "no_record");
        assert_eq!(api.call("person.setTodo", json!({ "id": "@I1@" })).unwrap_err().code, "bad_args");
        assert_eq!(depth(&api), depth0);
        // Taken by later tickets.
        assert_eq!(api.call("media.editPhoto", json!({ "id": "@O1@" })).unwrap_err().code, "unknown_method");
        assert_eq!(api.call("media.setCrop", json!({ "id": "@O1@" })).unwrap_err().code, "unknown_method");
    }

    #[test]
    fn a_saved_change_is_in_the_history_and_the_head_declares_the_tags() {
        let (dir, mut api) = open();
        api.call("person.setTodo", json!({ "id": "@I3@", "todo": true })).unwrap();
        api.call("media.setIgnored", json!({ "id": "@O1@", "ignored": true })).unwrap();
        api.call("archive.save", json!({ "author": "Ewa", "allowForeign": true })).unwrap();

        let saved = std::fs::read_to_string(dir.path().join("a/rodzina.ged")).unwrap();
        assert!(saved.contains("TAG _HLM_TODO") && saved.contains("TAG _HLM_IGNORE"), "{saved}");
        assert!(saved.contains("1 _HLM_TODO Y") && saved.contains("1 _HLM_IGNORE Y"));

        let feed = api.call("history.feed", json!({})).unwrap().to_string();
        assert!(feed.contains("Do uzupełnienia") && feed.contains("Pominięte w „Do uzupełnienia”"), "{feed}");
        let person = api.call("person.get", json!({ "id": "@I3@" })).unwrap();
        assert!(person["history"].to_string().contains("Do uzupełnienia"), "{}", person["history"]);
    }
}
