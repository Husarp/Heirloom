//! The app window: one command (`api`) that forwards to `heirloom-api` (`update.*` to its updater), and the
//! `heirloom://` protocol that serves the archive's photos and documents (PLAN.md §5.3). With `--selftest <report>`
//! the window is hidden, the interface checks itself (scripts/build.ps1 runs it before packaging) and the app quits.
//! While it runs, `%LOCALAPPDATA%\Heirloom\running\<pid>.json` tells the installer whether closing it would lose work.

use heirloom_api::{Api, ApiError, media, running::RunningFile, update::Updater};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{Manager, RunEvent};
use tauri::http::{Response, StatusCode, header::CONTENT_TYPE};

struct AppState(Mutex<Api>);

/// The self-test's report file, and a folder of its own for the throwaway archive and settings.
struct SelfTest(Option<(PathBuf, PathBuf)>);

/// The self-test's verdict is written once: by the interface, or by the watchdog when the interface is silent.
static VERDICT_WRITTEN: AtomicBool = AtomicBool::new(false);

fn finish_selftest(report: &Path, dir: &Path, text: &str, code: i32) {
    if VERDICT_WRITTEN.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::fs::write(report, text);
    let _ = std::fs::remove_dir_all(dir);
    std::process::exit(code);
}

/// Every UI command goes through here. `async` so it runs off the window's thread and never freezes it.
#[tauri::command]
async fn call(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    updater: tauri::State<'_, Updater>,
    method: String,
    args: Value,
) -> Result<Value, ApiError> {
    // Update checks wait on the internet, so they never take the archive's lock.
    if method.starts_with("update.") {
        let result = updater.call(&method, &args)?;
        if method == "update.install" {
            // The installer (started with --update) replaces the program folder, and Windows won't overwrite a running
            // exe: Heirloom goes; the installer starts it again.
            // The window has already asked about unsaved changes. A moment later, so this answer reaches it first.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                app.exit(0);
            });
        }
        return Ok(result);
    }
    let mut api = state.0.lock().unwrap_or_else(|e| e.into_inner());
    api.call(&method, args)
}

/// The window's close question would ask about this much (useCloseGuard.ts): written for the installer.
#[tauri::command]
async fn unsaved_state(
    running: tauri::State<'_, RunningFile>,
    unsaved_changes: u32,
    draft: bool,
    archive: Option<String>,
) -> Result<(), ApiError> {
    running.set(unsaved_changes, draft, archive);
    Ok(())
}

/// `taskkill` without /F - and anything else that closes a program politely - posts WM_CLOSE to every top-level
/// window of the process, also to the invisible one through which tao (Tauri's event loop) gets the messages other
/// threads send to the window's thread. Its default handling destroys that window, and from then on nothing sent
/// that way arrives: the window's own close never happens, whatever the close question was answered, and Heirloom
/// stays on screen half-alive until Task Manager ends it (0.4.0, installer run while Heirloom was open). That window
/// now ignores WM_CLOSE; the real window still asks about unsaved changes, as its ✕ does.
#[cfg(windows)]
mod close_shield {
    use std::sync::atomic::{AtomicIsize, Ordering};

    const GWLP_WNDPROC: i32 = -4;
    const WM_CLOSE: u32 = 0x0010;
    const TAO_TARGET: &str = "Tao Thread Event Target";
    static ORIGINAL: AtomicIsize = AtomicIsize::new(0);

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumThreadWindows(thread: u32, each: unsafe extern "system" fn(isize, isize) -> i32, param: isize) -> i32;
        fn GetClassNameW(window: isize, name: *mut u16, size: i32) -> i32;
        fn GetWindowLongPtrW(window: isize, index: i32) -> isize;
        fn SetWindowLongPtrW(window: isize, index: i32, value: isize) -> isize;
        fn CallWindowProcW(previous: isize, window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    unsafe extern "system" fn shielded(window: isize, message: u32, wparam: usize, lparam: isize) -> isize {
        if message == WM_CLOSE {
            return 0;
        }
        unsafe { CallWindowProcW(ORIGINAL.load(Ordering::Relaxed), window, message, wparam, lparam) }
    }

    unsafe extern "system" fn each(window: isize, _: isize) -> i32 {
        let mut name = [0u16; 64];
        let n = unsafe { GetClassNameW(window, name.as_mut_ptr(), name.len() as i32) }.max(0) as usize;
        if String::from_utf16_lossy(&name[..n]) != TAO_TARGET || ORIGINAL.load(Ordering::Relaxed) != 0 {
            return 1;
        }
        // (On the window's own thread: no message reaches it between these two lines.)
        ORIGINAL.store(unsafe { GetWindowLongPtrW(window, GWLP_WNDPROC) }, Ordering::Relaxed);
        unsafe { SetWindowLongPtrW(window, GWLP_WNDPROC, shielded as *const () as isize) };
        0
    }

    /// Called on the window's thread (Tauri's setup), where tao made that window.
    pub fn install() {
        unsafe { EnumThreadWindows(GetCurrentThreadId(), each, 0) };
    }
}

/// The folder the self-test may use, or nothing on a normal start.
#[tauri::command]
fn selftest_folder(test: tauri::State<'_, SelfTest>) -> Option<String> {
    test.0.as_ref().map(|(_, dir)| dir.join("archiwum").display().to_string())
}

/// The interface's verdict: written to the report, then the app quits.
#[tauri::command]
fn selftest_done(test: tauri::State<'_, SelfTest>, ok: bool, text: String) {
    if let Some((report, dir)) = &test.0 {
        finish_selftest(report, dir, &text, if ok { 0 } else { 1 });
    }
}

/// `--selftest <report>` from the command line.
fn selftest_args() -> Option<(PathBuf, PathBuf)> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--selftest" {
            let report = PathBuf::from(args.next()?);
            let dir = std::env::temp_dir().join(format!("heirloom-selftest-{}", std::process::id()));
            return Some((report, dir));
        }
    }
    None
}

pub fn run() {
    let selftest = selftest_args();
    // The self-test never touches the real settings, recent archives or thumbnails.
    let api = match &selftest {
        Some((_, dir)) => Api::new(Some(dir.join("ustawienia")), Some(dir.join("cache"))),
        None => Api::with_default_dirs(),
    };
    let roots = api.media_roots();
    // The self-test never goes online either.
    let updater = Updater::new(if selftest.is_some() { None } else { Some(Updater::default_dir()) });
    updater.remove_old_downloads();
    // The self-test is not a Heirloom the installer has to close.
    let running = RunningFile::new(if selftest.is_some() { None } else { RunningFile::default_dir() });
    let watchdog = selftest.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState(Mutex::new(api)))
        .manage(updater)
        .manage(running)
        .manage(SelfTest(selftest))
        .register_asynchronous_uri_scheme_protocol("heirloom", move |_ctx, request, responder| {
            let roots = roots.clone();
            let path = request.uri().path().trim_start_matches('/').to_string();
            // Thumbnails can take a moment to make; never block the window while they are made.
            std::thread::spawn(move || {
                let response = match media::serve(&roots, &path) {
                    media::Served::Ok { body, mime } => Response::builder().header(CONTENT_TYPE, mime).body(body),
                    media::Served::NotFound => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
                    media::Served::BadRequest => Response::builder().status(StatusCode::BAD_REQUEST).body(Vec::new()),
                };
                responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
            });
        })
        .setup(move |app| {
            #[cfg(windows)]
            close_shield::install();
            // A normal start shows the window as tauri.conf.json makes it; only the self-test hides it.
            if let Some((report, dir)) = watchdog {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                // An interface that never answers (no WebView2, a script error before the test) is a failure too.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(60));
                    finish_selftest(&report, &dir, "FAILED: the window gave no answer within 60 s", 2);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, unsaved_state, selftest_folder, selftest_done])
        .build(tauri::generate_context!())
        .expect("error while running Heirloom")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<RunningFile>().remove();
            }
        });
}
