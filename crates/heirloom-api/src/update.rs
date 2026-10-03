//! Update checks and updates from inside the app (APP-STANDARDS.md §2–3). The only things Heirloom ever sends over
//! the internet: the question „what is the newest version?” to GitHub (nothing about the family or the archive goes
//! with it), and, after a click on „Aktualizuj”, the download of the new installer.
//!
//! Kept apart from `Api`: a request can take up to 10 s, and must not hold the lock every other command waits on.
//! The window (`src-tauri`) and the bridge send every `update.*` command here.

use crate::ApiError;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const LATEST_URL: &str = "https://api.github.com/repos/Husarp/Heirloom/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/Husarp/Heirloom/releases";
/// This app downloads and RUNS what it gets, so the installer must come from Heirloom's own releases.
const DOWNLOAD_PREFIX: &str = "https://github.com/Husarp/Heirloom/releases/download/";
/// Automatic checks ask GitHub at most this often: it answers only 60 unsigned requests an hour.
pub const MIN_GAP: Duration = Duration::from_secs(5 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A version as numbers, so 0.10.0 is newer than 0.9.3 (a text compare gets that wrong). „v0.4.0” and „0.4” work too.
pub fn version_key(text: &str) -> Vec<u64> {
    text.trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|piece| piece.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0))
        .collect()
}

/// Is `latest` newer than `current`? Missing parts count as 0 (0.4 = 0.4.0).
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (version_key(latest), version_key(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// When automatic checks last asked GitHub.
#[derive(Debug, Default)]
pub struct Throttle(Option<Instant>);

impl Throttle {
    /// May an automatic check ask GitHub now?
    pub fn due(&self, now: Instant) -> bool {
        self.0.is_none_or(|last| now.saturating_duration_since(last) >= MIN_GAP)
    }

    /// A check that reached GitHub. One that never got through (no internet) cost nothing there, so it doesn't
    /// count, and the next try may come at once.
    pub fn asked(&mut self, now: Instant) {
        self.0 = Some(now);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    /// The release's page on GitHub.
    pub page: String,
    /// The Windows installer: the release's first `.exe`, found by its ending, never by an exact name.
    pub installer: Option<Installer>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Installer {
    pub url: String,
    pub size: u64,
}

/// GitHub's answer for `/releases/latest`; None when it isn't one.
pub fn parse_release(body: &str) -> Option<Release> {
    let data: Value = serde_json::from_str(body).ok()?;
    let version = data["tag_name"].as_str()?.trim().trim_start_matches(['v', 'V']).to_string();
    if !plain_version(&version) {
        return None;
    }
    let installer = data["assets"].as_array().into_iter().flatten().find_map(|asset| {
        let name = asset["name"].as_str()?.to_lowercase();
        let url = asset["browser_download_url"].as_str()?;
        (name.ends_with(".exe") && url.starts_with(DOWNLOAD_PREFIX))
            .then(|| Installer { url: url.to_string(), size: asset["size"].as_u64().unwrap_or(0) })
    });
    let page = data["html_url"].as_str().filter(|p| p.starts_with(RELEASES_PAGE)).unwrap_or(RELEASES_PAGE).to_string();
    Some(Release { version, page, installer })
}

/// „0.4.0”: only digits and dots, because it becomes part of a file name.
fn plain_version(text: &str) -> bool {
    !text.is_empty() && text.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

enum Download {
    None,
    Running { version: String, done: u64, total: u64 },
    Ready { version: String, path: PathBuf },
    Failed { version: String, message: String },
}

struct State {
    throttle: Throttle,
    /// An automatic check is under way (focus and „window shown” often come together: one request is enough).
    checking: bool,
    latest: Option<Release>,
    download: Download,
}

impl State {
    fn finish_check(&mut self, result: Result<Release, Failure>, now: Instant) -> Result<(), ApiError> {
        self.checking = false;
        match result {
            Ok(release) => {
                self.throttle.asked(now);
                self.latest = Some(release);
                Ok(())
            }
            Err(failure) => {
                if failure.reached {
                    self.throttle.asked(now);
                }
                Err(failure.error)
            }
        }
    }
}

pub struct Updater {
    /// Where the installer is downloaded to; None switches updates off (the self-test never goes online).
    dir: Option<PathBuf>,
    state: Arc<Mutex<State>>,
}

impl Updater {
    pub fn new(dir: Option<PathBuf>) -> Updater {
        let state = State { throttle: Throttle::default(), checking: false, latest: None, download: Download::None };
        Updater { dir, state: Arc::new(Mutex::new(state)) }
    }

    /// `%TEMP%\Heirloom-aktualizacja`.
    pub fn default_dir() -> PathBuf {
        std::env::temp_dir().join("Heirloom-aktualizacja")
    }

    /// The installer that brought this version has done its job: it goes at the next start. Right after an update
    /// the installer may still be closing (it started Heirloom), so whatever is still locked is tried again a
    /// minute later; only those files, never a download started since.
    pub fn remove_old_downloads(&self) {
        let Some(dir) = self.dir.clone() else { return };
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        let old: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        if old.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            let left: Vec<&PathBuf> = old.iter().filter(|p| std::fs::remove_file(p).is_err()).collect();
            if !left.is_empty() {
                std::thread::sleep(Duration::from_secs(60));
                for path in left {
                    let _ = std::fs::remove_file(path);
                }
            }
        });
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn call(&self, method: &str, args: &Value) -> Result<Value, ApiError> {
        match method {
            "update.status" => Ok(self.status()),
            "update.check" => self.check(args.get("manual").and_then(Value::as_bool).unwrap_or(false)),
            "update.download" => self.download(),
            "update.install" => self.install(),
            _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
        }
    }

    /// What the window shows: this version, the newest one known, and how the download is going.
    pub fn status(&self) -> Value {
        let s = self.lock();
        let latest = s.latest.as_ref();
        let download = match &s.download {
            Download::None => json!({ "state": "none" }),
            Download::Running { version, done, total } => json!({ "state": "running", "version": version, "done": done, "total": total }),
            Download::Ready { version, .. } => json!({ "state": "ready", "version": version }),
            Download::Failed { version, message } => json!({ "state": "failed", "version": version, "message": message }),
        };
        json!({
            "enabled": self.dir.is_some(),
            "current": VERSION,
            "checked": latest.is_some(),
            "latest": latest.map(|r| &r.version),
            "newer": latest.is_some_and(|r| is_newer(&r.version, VERSION)),
            "installer": latest.is_some_and(|r| r.installer.is_some()),
            "page": latest.map_or(RELEASES_PAGE, |r| &r.page),
            "download": download,
        })
    }

    /// `manual`: „Sprawdź teraz” asks GitHub every time; an automatic check at most every 5 minutes, and otherwise
    /// answers with what the last one learned.
    fn check(&self, manual: bool) -> Result<Value, ApiError> {
        if self.dir.is_none() {
            return Ok(self.status());
        }
        {
            let mut s = self.lock();
            if !manual && (s.checking || !s.throttle.due(Instant::now())) {
                drop(s);
                return Ok(self.status());
            }
            s.checking = true;
        }
        let result = fetch_latest();
        self.lock().finish_check(result, Instant::now())?;
        Ok(self.status())
    }

    /// Starts downloading the newest version's installer in the background; `update.status` shows the progress.
    fn download(&self) -> Result<Value, ApiError> {
        let dir = self.dir.clone().ok_or_else(|| ApiError::new("updates_off", "Aktualizacje są tu wyłączone."))?;
        let mut s = self.lock();
        let release = s.latest.clone().filter(|r| is_newer(&r.version, VERSION));
        let Some(release) = release else {
            return Err(ApiError::new("no_update", "Nie ma nowszej wersji do pobrania. Sprawdź jeszcze raz."));
        };
        match &s.download {
            Download::Running { version, .. } if *version == release.version => {
                drop(s);
                return Ok(self.status());
            }
            Download::Ready { version, path } if *version == release.version && path.is_file() => {
                drop(s);
                return Ok(self.status());
            }
            _ => {}
        }
        let Some(installer) = release.installer else {
            return Err(ApiError::new(
                "no_installer",
                format!("Wersja {} nie ma jeszcze instalatora dla Windows. Zajrzyj na stronę wydań („GitHub”).", release.version),
            ));
        };
        let version = release.version;
        s.download = Download::Running { version: version.clone(), done: 0, total: installer.size };
        drop(s);
        let state = self.state.clone();
        std::thread::spawn(move || {
            let progress = |done: u64, total: u64| {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                s.download = Download::Running { version: version.clone(), done, total };
            };
            let result = fetch_installer(&installer, &dir, &version, progress);
            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
            s.download = match result {
                Ok(path) => Download::Ready { version, path },
                Err(message) => Download::Failed { version, message },
            };
        });
        Ok(self.status())
    }

    /// Starts the downloaded installer. The caller then closes Heirloom, so the installer can replace it.
    fn install(&self) -> Result<Value, ApiError> {
        let mut s = self.lock();
        let Download::Ready { path, .. } = &s.download else {
            return Err(ApiError::new("not_ready", "Aktualizacja nie jest jeszcze pobrana."));
        };
        if !path.is_file() {
            s.download = Download::None;
            return Err(ApiError::new("not_ready", "Pobrany instalator zniknął z dysku. Pobierz aktualizację jeszcze raz."));
        }
        std::process::Command::new(path)
            .spawn()
            .map_err(|e| ApiError::new("install_failed", format!("Nie udało się uruchomić instalatora ({e}).")))?;
        Ok(json!({ "started": true }))
    }
}

struct Failure {
    /// The request got an answer from GitHub (then it counts toward the 5 minutes).
    reached: bool,
    error: ApiError,
}

/// The settings both requests share; each adds its own timeouts.
fn config() -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope> {
    use ureq::tls::{RootCerts, TlsConfig};
    // Windows' own certificate store, the one the browser trusts: where Edge reaches GitHub, Heirloom does too (also
    // behind an antivirus or a company network that checks secure connections).
    let tls = TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder().http_status_as_error(false).user_agent(format!("Heirloom/{VERSION}")).tls_config(tls)
}

fn fetch_latest() -> Result<Release, Failure> {
    let agent: ureq::Agent = config().timeout_global(Some(CHECK_TIMEOUT)).build().into();
    let mut response = agent
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| Failure { reached: false, error: connection_error(&e) })?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(Failure { reached: true, error: status_error(status) });
    }
    let unreadable = || ApiError::new("bad_answer", "GitHub odpowiedział czymś niezrozumiałym. Spróbuj później albo zajrzyj na stronę wydań („GitHub”).");
    let body = response.body_mut().with_config().limit(4 << 20).read_to_string().map_err(|e| {
        let error = if matches!(e, ureq::Error::Timeout(_)) { connection_error(&e) } else { unreadable() };
        Failure { reached: true, error }
    })?;
    parse_release(&body).ok_or(Failure { reached: true, error: unreadable() })
}

/// Downloads to `HeirloomSetup-X.Y.Z.exe.part`, renamed only once complete; Err is a message for the family.
fn fetch_installer(installer: &Installer, dir: &Path, version: &str, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    let target = dir.join(format!("HeirloomSetup-{version}.exe"));
    let part = dir.join(format!("HeirloomSetup-{version}.exe.part"));
    let disk = |e: std::io::Error| format!("Nie udało się zapisać instalatora w {} ({e}).", dir.display());
    std::fs::create_dir_all(dir).map_err(disk)?;
    let agent: ureq::Agent = config()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(15 * 60)))
        .build()
        .into();
    let mut response = agent.get(&installer.url).call().map_err(|e| connection_error(&e).message)?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(status_error(status).message);
    }
    let total = response.body().content_length().unwrap_or(installer.size);
    let mut reader = response.body_mut().as_reader();
    let mut out = std::fs::File::create(&part).map_err(disk)?;
    let mut buffer = vec![0u8; 1 << 16];
    let mut done = 0u64;
    progress(0, total);
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                drop(out);
                let _ = std::fs::remove_file(&part);
                return Err(read_error(e));
            }
        };
        out.write_all(&buffer[..n]).map_err(disk)?;
        done += n as u64;
        progress(done, total);
    }
    out.flush().map_err(disk)?;
    drop(out);
    // A cut-off download would fail later as a baffling „not a valid Win32 application”.
    let expected = if installer.size > 0 { installer.size } else { total };
    if done < 1_000_000 || (expected > 0 && done != expected) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Pobieranie przerwało się w połowie ({} z {} MB). Spróbuj ponownie.", mb(done), mb(expected)));
    }
    let _ = std::fs::remove_file(&target);
    std::fs::rename(&part, &target).map_err(disk)?;
    Ok(target)
}

fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.0).replace('.', ",")
}

fn read_error(e: std::io::Error) -> String {
    match e.into_inner().and_then(|inner| inner.downcast::<ureq::Error>().ok()) {
        Some(e) => connection_error(&e).message,
        None => "Połączenie zerwało się w trakcie pobierania. Spróbuj ponownie.".into(),
    }
}

/// What a failed connection most likely means, in words (never „Failed to fetch”).
fn connection_error(e: &ureq::Error) -> ApiError {
    match e {
        ureq::Error::Timeout(_) => ApiError::new(
            "timeout",
            "GitHub nie odpowiedział w ciągu 10 sekund. Połączenie jest bardzo wolne albo coś je blokuje (zapora, VPN, program antywirusowy).",
        ),
        ureq::Error::HostNotFound => ApiError::new("offline", "Brak połączenia z internetem: nie udało się odnaleźć serwera GitHub."),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => ApiError::new(
            "tls",
            "Nie udało się bezpiecznie połączyć z GitHubem. Najczęściej to zła data lub godzina w komputerze albo program antywirusowy, który sprawdza połączenia.",
        ),
        _ => ApiError::new("offline", "Nie udało się połączyć z GitHubem. Nie ma internetu albo połączenie blokuje zapora, VPN lub program antywirusowy."),
    }
}

fn status_error(status: u16) -> ApiError {
    match status {
        404 => ApiError::new("no_release", "Na GitHubie nie ma jeszcze wydania Heirloom do pobrania."),
        403 | 429 => ApiError::new("rate_limited", "GitHub chwilowo nie odpowiada na pytania z tej sieci (ma limit na godzinę). Spróbuj za godzinę."),
        500.. => ApiError::new("github_down", format!("GitHub ma chwilowe kłopoty (błąd {status}). Spróbuj później.")),
        _ => ApiError::new("bad_answer", format!("GitHub odpowiedział nieoczekiwanie (kod {status}). Spróbuj później.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("3.10.0", "3.9.0"));
        assert!(is_newer("v0.4.0", "0.3.3"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(is_newer("0.3.10", "0.3.9"));
        assert!(!is_newer("0.3.3", "0.3.3"));
        assert!(!is_newer("0.4", "0.4.0"));
        assert!(!is_newer("0.4.0", "0.4"));
        assert!(!is_newer("0.3.2", "0.3.10"));
        assert!(!is_newer("", "0.3.3"));
        assert_eq!(version_key("v1.2.3"), [1, 2, 3]);
    }

    #[test]
    fn automatic_checks_ask_github_at_most_every_5_minutes() {
        let start = Instant::now();
        let mut throttle = Throttle::default();
        assert!(throttle.due(start), "the first check goes at once");
        throttle.asked(start);
        assert!(!throttle.due(start));
        assert!(!throttle.due(start + Duration::from_secs(4 * 60 + 59)));
        assert!(throttle.due(start + MIN_GAP));
        assert!(throttle.due(start + Duration::from_secs(60 * 60)));
        // a clock that seems to go back never blocks checks for good
        throttle.asked(start + Duration::from_secs(10));
        assert!(!throttle.due(start));
    }

    #[test]
    fn only_a_check_that_reached_github_counts_toward_the_5_minutes() {
        let updater = Updater::new(None);
        let mut s = updater.lock();
        let now = Instant::now();
        let offline = Failure { reached: false, error: ApiError::new("offline", "") };
        assert_eq!(s.finish_check(Err(offline), now).unwrap_err().code, "offline");
        assert!(s.throttle.due(now), "no internet: the next check may go at once");
        let refused = Failure { reached: true, error: status_error(429) };
        assert_eq!(s.finish_check(Err(refused), now).unwrap_err().code, "rate_limited");
        assert!(!s.throttle.due(now));
        let release = Release { version: "99.0.0".into(), page: RELEASES_PAGE.into(), installer: None };
        s.finish_check(Ok(release), now + MIN_GAP).unwrap();
        assert!(!s.throttle.due(now + MIN_GAP));
        drop(s);
        assert_eq!(updater.status()["newer"], true);
        assert_eq!(updater.status()["latest"], "99.0.0");
    }

    #[test]
    fn the_installer_is_found_by_its_ending() {
        let body = r#"{
            "tag_name": "v0.4.0",
            "html_url": "https://github.com/Husarp/Heirloom/releases/tag/v0.4.0",
            "assets": [
                { "name": "BUILT.json", "size": 120, "browser_download_url": "https://github.com/Husarp/Heirloom/releases/download/v0.4.0/BUILT.json" },
                { "name": "HeirloomSetup-0.4.0.EXE", "size": 21000000, "browser_download_url": "https://github.com/Husarp/Heirloom/releases/download/v0.4.0/HeirloomSetup-0.4.0.EXE" }
            ]
        }"#;
        let release = parse_release(body).unwrap();
        assert_eq!(release.version, "0.4.0");
        assert_eq!(release.page, "https://github.com/Husarp/Heirloom/releases/tag/v0.4.0");
        let installer = release.installer.unwrap();
        assert!(installer.url.ends_with("HeirloomSetup-0.4.0.EXE"));
        assert_eq!(installer.size, 21_000_000);
    }

    #[test]
    fn an_installer_from_elsewhere_or_an_odd_version_is_refused() {
        let elsewhere = r#"{ "tag_name": "v0.4.0", "assets": [
            { "name": "HeirloomSetup-0.4.0.exe", "size": 1, "browser_download_url": "https://example.com/HeirloomSetup-0.4.0.exe" } ] }"#;
        let release = parse_release(elsewhere).unwrap();
        assert_eq!(release.installer, None);
        assert_eq!(release.page, RELEASES_PAGE);
        assert_eq!(parse_release(r#"{ "tag_name": "v0.4.0-beta" }"#), None);
        assert_eq!(parse_release(r#"{ "tag_name": "../../x" }"#), None);
        assert_eq!(parse_release("not json"), None);
    }

    #[test]
    fn switched_off_updater_never_goes_online() {
        let updater = Updater::new(None);
        let status = updater.call("update.check", &json!({ "manual": true })).unwrap();
        assert_eq!(status["enabled"], false);
        assert_eq!(status["checked"], false);
        assert_eq!(status["current"], VERSION);
        assert_eq!(updater.call("update.download", &Value::Null).unwrap_err().code, "updates_off");
        assert_eq!(updater.call("update.install", &Value::Null).unwrap_err().code, "not_ready");
    }

    #[test]
    fn old_downloads_go_at_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("HeirloomSetup-0.3.3.exe");
        std::fs::write(&old, b"x").unwrap();
        Updater::new(Some(dir.path().to_path_buf())).remove_old_downloads();
        for _ in 0..100 {
            if !old.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!old.exists());
    }
}
