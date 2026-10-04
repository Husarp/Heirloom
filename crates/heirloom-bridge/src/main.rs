//! `heirloom-bridge [--port 1430] [--open <archive or .heirloom-zestaw>]`
//!
//! Development only. Serves the same commands as the app window over HTTP on 127.0.0.1, so the UI can run in a
//! normal browser (`npm run dev`, then http://localhost:1420). Vite forwards `/api` and `/media` here.
//! It uses its own settings folder (`%TEMP%\heirloom-bridge`), so it never touches the app's recent list.

use heirloom_api::{Api, ApiError, media, update::Updater};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server};

fn main() {
    let mut port = 1430;
    let mut open: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().and_then(|p| p.parse().ok()).expect("--port needs a number"),
            "--open" => open = args.next(),
            _ => panic!("unknown argument {arg}"),
        }
    }
    let dir = std::env::temp_dir().join("heirloom-bridge");
    let mut api = Api::new(Some(dir.join("config")), Some(dir.join("cache")));
    if let Some(path) = open {
        if let Err(e) = api.open_path(&path) {
            eprintln!("could not open {path}: {}", e.message);
        }
    }
    let media_roots = api.media_roots();
    let updater = Updater::new(Some(dir.join("aktualizacja")));
    let server = Server::http(("127.0.0.1", port)).expect("could not listen");
    println!("heirloom-bridge on http://127.0.0.1:{port}");

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let header = |name: &'static str| request.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string());
        let response = if !allowed(header("Host").as_deref(), header("Origin").as_deref()) {
            Response::from_string("forbidden").with_status_code(403)
        } else if let (Method::Post, Some(method)) = (request.method(), url.strip_prefix("/api/")) {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let args: Value = if body.trim().is_empty() { Value::Null } else { serde_json::from_str(&body).unwrap_or(Value::Null) };
            let result = if method == "update.install" {
                Err(ApiError::new("install_failed", "Aktualizację instaluje tylko okno programu."))
            } else if method.starts_with("update.") {
                updater.call(method, &args)
            } else {
                api.call(method, args)
            };
            let result = match result {
                Ok(value) => json!({ "ok": value }),
                Err(error) => json!({ "error": error }),
            };
            Response::from_data(serde_json::to_vec(&result).unwrap_or_default())
                .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        } else if let (Method::Get, Some(path)) = (request.method(), url.strip_prefix("/media/")) {
            match media::serve(&media_roots, path) {
                media::Served::Ok { body, mime } => {
                    Response::from_data(body).with_header(Header::from_bytes("Content-Type", mime).unwrap())
                }
                media::Served::NotFound => Response::from_string("not found").with_status_code(404),
                media::Served::BadRequest => Response::from_string("bad request").with_status_code(400),
            }
        } else {
            Response::from_string("not found").with_status_code(404)
        };
        let _ = request.respond(response);
    }
}

/// Only pages of this computer may use the bridge (the UI through Vite's proxy, curl). A web page from elsewhere could
/// otherwise send it commands (a browser sends a simple POST without asking), or, pointing its own name at 127.0.0.1,
/// read the archive and any file it hands to the import.
fn allowed(host: Option<&str>, origin: Option<&str>) -> bool {
    let local = |authority: &str| {
        let name = match authority.strip_prefix('[') {
            Some(rest) => rest.split(']').next().unwrap_or(""),
            None => authority.split(':').next().unwrap_or(""),
        };
        matches!(name, "127.0.0.1" | "localhost" | "::1")
    };
    host.is_some_and(local) && origin.is_none_or(|o| o.strip_prefix("http://").is_some_and(local))
}

#[cfg(test)]
mod tests {
    use super::allowed;

    #[test]
    fn only_local_pages_may_use_the_bridge() {
        assert!(allowed(Some("127.0.0.1:1430"), None), "curl");
        assert!(allowed(Some("localhost:1420"), Some("http://localhost:1420")), "the UI through Vite's proxy");
        assert!(allowed(Some("[::1]:1430"), Some("http://[::1]:1420")));
        assert!(!allowed(Some("127.0.0.1:1430"), Some("https://example.com")), "a page elsewhere sending commands");
        assert!(!allowed(Some("evil.example:1430"), Some("http://evil.example:1430")), "DNS rebinding");
        assert!(!allowed(Some("localhost.evil.example:1430"), None));
        assert!(!allowed(Some("127.0.0.1:1430"), Some("null")));
        assert!(!allowed(None, None));
    }
}
