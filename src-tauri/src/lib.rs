//! The app window: one command (`api`) that forwards to `heirloom-api`, and the `heirloom://` protocol that
//! serves the archive's photos and documents (PLAN.md §5.3). With `--selftest <report>` the window is hidden, the
//! interface checks itself (scripts/build.ps1 runs it before packaging) and the app quits.

use heirloom_api::{Api, ApiError, media};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;
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
async fn call(state: tauri::State<'_, AppState>, method: String, args: Value) -> Result<Value, ApiError> {
    let mut api = state.0.lock().unwrap_or_else(|e| e.into_inner());
    api.call(&method, args)
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
    let watchdog = selftest.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState(Mutex::new(api)))
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
        .invoke_handler(tauri::generate_handler![call, selftest_folder, selftest_done])
        .run(tauri::generate_context!())
        .expect("error while running Heirloom");
}
