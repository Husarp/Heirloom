//! File moves and copies that never replace an existing file, and names that are safe as Windows folder names.
//! `std::fs::rename` and `fs::copy` replace a file already at the target (on Windows too): every move and copy into a
//! folder the user keeps (the archive, „Moje zmiany”) goes through here instead.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Moves `from` to `to`, failing with `AlreadyExists` when something is there. Nothing changes on any failure.
pub fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::move_file(from, to)
    }
    #[cfg(not(windows))]
    {
        match fs::hard_link(from, to) {
            Ok(()) => fs::remove_file(from).inspect_err(|_| {
                let _ = fs::remove_file(to);
            }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
            Err(_) => {
                // A file system without hard links (or a folder): checked, then renamed.
                if fs::symlink_metadata(to).is_ok() {
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists, "the target exists"));
                }
                fs::rename(from, to)
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    /// Verbatim (`\\?\`) paths: long paths and names Windows would otherwise change (trailing dots) stay as they are.
    fn verbatim(path: &Path, parent_only: bool) -> io::Result<Vec<u16>> {
        let full = if parent_only {
            let parent = path.parent().ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
            let name = path.file_name().ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
            std::fs::canonicalize(parent)?.join(name)
        } else {
            std::fs::canonicalize(path)?
        };
        Ok(full.as_os_str().encode_wide().chain(std::iter::once(0)).collect())
    }

    pub fn move_file(from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (verbatim(from, false)?, verbatim(to, true)?);
        // Flags 0: no MOVEFILE_REPLACE_EXISTING, so an existing target is an error, never replaced.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } != 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(80 | 183) => Err(io::Error::new(io::ErrorKind::AlreadyExists, error)),
            _ => Err(error),
        }
    }
}

/// Copies `from` to `to` (a new file; `AlreadyExists` when something is there): first to a temporary name next to it,
/// written to the disk, then moved into place. Returns the copy's hash.
pub fn copy_no_replace(from: &Path, to: &Path) -> io::Result<blake3::Hash> {
    let name = to.file_name().map_or_else(|| "plik".into(), |n| n.to_string_lossy().into_owned());
    let temp = to.with_file_name(format!("{name}.tmp-{}", std::process::id()));
    let hash = (|| {
        let mut source = fs::File::open(from)?;
        let mut target = fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            target.write_all(&buffer[..n])?;
        }
        target.sync_all()?;
        Ok::<_, io::Error>(hasher.finalize())
    })();
    let result = hash.and_then(|hash| move_no_replace(&temp, to).map(|()| hash));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// A file's hash, read in pieces.
pub fn hash_file(path: &Path) -> io::Result<blake3::Hash> {
    let mut file = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            return Ok(hasher.finalize());
        }
        hasher.update(&buffer[..n]);
    }
}

/// How names are compared: NTFS ignores case, and a Mac-made stick writes Polish letters decomposed (NFD).
pub fn name_key(name: &str) -> String {
    name.nfc().collect::<String>().to_lowercase()
}

/// `stem.ext`, or `stem (2).ext`, `stem (3).ext` … — the first name `taken` doesn't have (compared with [`name_key`]).
pub fn free_name(taken: &dyn Fn(&str) -> bool, stem: &str, ext: &str) -> String {
    let dot = if ext.is_empty() { String::new() } else { format!(".{ext}") };
    let mut name = format!("{stem}{dot}");
    let mut n = 2;
    while taken(&name_key(&name)) {
        name = format!("{stem} ({n}){dot}");
        n += 1;
    }
    name
}

/// A name Windows keeps for a device, with any extension: `CON`, `nul.txt`, `COM1.jpg`, `LPT¹`.
pub(crate) fn is_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ').to_ascii_uppercase();
    let numbered = |prefix: &str| stem.strip_prefix(prefix).is_some_and(|n| matches!(n, "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"));
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") || numbered("COM") || numbered("LPT")
}

/// A text as a folder name Windows accepts: forbidden characters become spaces, no trailing dots or spaces, no device
/// names, at most 60 characters. Polish letters stay.
pub fn folder_name(text: &str) -> String {
    let replaced: String = text.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c }).collect();
    let mut name = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().count() > 60 {
        name = name.chars().take(60).collect();
    }
    let name = name.trim_end_matches(['.', ' ']).to_string();
    if name.is_empty() {
        return "bez nazwy".into();
    }
    if is_device_name(&name) { format!("{name} (folder)") } else { name }
}

/// Files other programs leave in folders (thumbnail caches, folder settings, Office locks, our own temporary files).
pub fn is_noise(name: &str) -> bool {
    let lower = name.to_lowercase();
    if matches!(lower.as_str(), "thumbs.db" | "desktop.ini" | ".ds_store") || name.starts_with("._") || name.starts_with("~$") {
        return true;
    }
    lower.rsplit_once(".tmp-").is_some_and(|(_, pid)| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

/// Runs `op` again once, 300 ms later, when another program held the file (Windows errors 32 and 33).
pub fn with_retry<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    match op() {
        Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) && cfg!(windows) => {
            std::thread::sleep(std::time::Duration::from_millis(300));
            op()
        }
        other => other,
    }
}

/// Every file under `dir` (relative path with `/`, full path), links not followed.
pub fn walk(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => stack.push(entry.path()),
                Ok(kind) if kind.is_file() => {
                    let path = entry.path();
                    if let Ok(relative) = path.strip_prefix(dir) {
                        let relative = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                        files.push((relative, path));
                    }
                }
                _ => {}
            }
        }
    }
    files.sort();
    files
}

/// Clears the read-only mark (files copied from a CD keep it), so our own copy can be deleted.
pub fn remove_own_file(path: &Path) -> io::Result<()> {
    if let Ok(meta) = fs::metadata(path) {
        let mut permissions = meta.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
    fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_never_replaces_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.jpg"), dir.path().join("b.jpg"));
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        assert_eq!(move_no_replace(&a, &b).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!((fs::read(&a).unwrap(), fs::read(&b).unwrap()), (b"a".to_vec(), b"b".to_vec()), "nothing moved");
        let c = dir.path().join("c.jpg");
        move_no_replace(&a, &c).unwrap();
        assert!(!a.exists() && fs::read(&c).unwrap() == b"a");
    }

    #[test]
    fn copying_never_replaces_a_file_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.jpg"), dir.path().join("b.jpg"));
        fs::write(&a, b"zdjecie").unwrap();
        fs::write(&b, b"inne").unwrap();
        assert_eq!(copy_no_replace(&a, &b).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&b).unwrap(), b"inne");
        let c = dir.path().join("c.jpg");
        assert_eq!(copy_no_replace(&a, &c).unwrap(), blake3::hash(b"zdjecie"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3, "no .tmp- file left");
    }

    #[cfg(windows)]
    #[test]
    fn a_file_held_by_another_program_is_not_moved() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.jpg");
        fs::write(&a, b"a").unwrap();
        let _held = fs::OpenOptions::new().read(true).share_mode(0).open(&a).unwrap();
        let error = move_no_replace(&a, &dir.path().join("b.jpg")).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        assert!(a.exists() && !dir.path().join("b.jpg").exists());
    }

    #[test]
    fn folder_names_are_safe_on_windows() {
        assert_eq!(folder_name("Rodzina: Kowalskich?"), "Rodzina Kowalskich");
        assert_eq!(folder_name("Józef Kowalski..."), "Józef Kowalski");
        assert_eq!(folder_name("  "), "bez nazwy");
        assert_eq!(folder_name("CON"), "CON (folder)");
        assert_eq!(folder_name("nul.txt"), "nul.txt (folder)");
        let long = "Ż".repeat(80);
        assert_eq!(folder_name(&long).chars().count(), 60);
    }

    #[test]
    fn free_names_compare_like_windows() {
        let taken = ["ślub.jpg".to_string(), "ślub (2).jpg".to_string()];
        let name = free_name(&|n| taken.contains(&n.to_string()), "Ślub", "jpg");
        assert_eq!(name, "Ślub (3).jpg");
        // A decomposed ś (a Mac-made stick) is the same name.
        assert_eq!(name_key("s\u{301}lub.JPG"), "ślub.jpg");
    }

    #[test]
    fn noise_files_are_recognised() {
        for name in ["Thumbs.db", "desktop.ini", ".DS_Store", "._x.jpg", "~$list.docx", "x.jpg.tmp-1234"] {
            assert!(is_noise(name), "{name}");
        }
        assert!(!is_noise("x.jpg") && !is_noise("x.tmp-a"));
    }
}
