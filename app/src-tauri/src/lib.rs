mod bridge;
mod cookies;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::bridge::Bridge;
use crate::cookies::{DetectedBrowser, ImportedCookies};

struct AppState {
    bridge: Arc<Bridge>,
}

#[tauri::command]
async fn bridge_status(state: State<'_, AppState>) -> Result<Value, String> {
    state.bridge.call("status", None).await
}

#[tauri::command]
async fn bridge_pair(state: State<'_, AppState>) -> Result<Value, String> {
    state.bridge.call("pair", None).await
}

#[tauri::command]
async fn bridge_connect(state: State<'_, AppState>) -> Result<Value, String> {
    state.bridge.call("connect", None).await
}

#[tauri::command]
async fn bridge_unpair(state: State<'_, AppState>) -> Result<Value, String> {
    state.bridge.call("unpair", None).await
}

#[tauri::command]
async fn bridge_list_conversations(
    state: State<'_, AppState>,
    count: Option<u32>,
) -> Result<Value, String> {
    let params = count.map(|c| json!({ "count": c }));
    state.bridge.call("list_conversations", params).await
}

#[tauri::command]
async fn bridge_fetch_messages(
    state: State<'_, AppState>,
    conversation_id: String,
    count: Option<u32>,
    cursor: Option<Value>,
) -> Result<Value, String> {
    let mut params = json!({ "conversation_id": conversation_id });
    if let Some(c) = count {
        params["count"] = json!(c);
    }
    if let Some(cur) = cursor {
        params["cursor"] = cur;
    }
    state.bridge.call("fetch_messages", Some(params)).await
}

/// Forward a user-pasted cookie map straight to the bridge. Used because
/// Google blocks login from WebKitGTK, so the embedded `start_gaia_login`
/// flow can't actually complete sign-in.
#[tauri::command]
async fn pair_with_cookies(
    state: State<'_, AppState>,
    cookies: HashMap<String, String>,
) -> Result<Value, String> {
    let params = json!({ "cookies": cookies });
    state.bridge.call("pair_gaia", Some(params)).await
}

/// Probe local browsers for google.com cookies and return the ones that
/// have data. Used to populate the "Import from browser" picker.
#[tauri::command]
async fn detect_browsers() -> Result<Vec<DetectedBrowser>, String> {
    tauri::async_runtime::spawn_blocking(cookies::detect)
        .await
        .map_err(|e| e.to_string())
}

/// Pull google.com cookies from `browser` (e.g. "brave", "chrome").
/// Caller is expected to forward the result to `pair_with_cookies`.
#[tauri::command]
async fn import_browser_cookies(browser: String) -> Result<ImportedCookies, String> {
    tauri::async_runtime::spawn_blocking(move || cookies::import(&browser))
        .await
        .map_err(|e| e.to_string())?
}

/// Open a webview window pointed at messages.google.com so the user can
/// sign into Google. When we detect a logged-in landing on
/// messages.google.com (cookies present), we harvest the cookies, close
/// the window, and forward to the bridge's pair_gaia method.
#[tauri::command]
async fn start_gaia_login(app: AppHandle) -> Result<(), String> {
    let triggered = Arc::new(Mutex::new(false));
    let triggered_cb = triggered.clone();
    let app_cb = app.clone();

    let url: url::Url = "https://messages.google.com/web/authentication"
        .parse()
        .map_err(|e: url::ParseError| e.to_string())?;

    // Google serves a blank page to WebKitGTK's default UA — masquerade as
    // recent Chrome on Linux so the login flow renders.
    const CHROME_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                             (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

    WebviewWindowBuilder::new(&app, "gaia-login", WebviewUrl::External(url))
        .title("Sign in to Google")
        .inner_size(520.0, 720.0)
        .user_agent(CHROME_UA)
        .on_navigation(|url| {
            log::info!("gaia-login navigating: {url}");
            true
        })
        .on_page_load(move |webview, payload| {
            let url = payload.url();
            if url.host_str().unwrap_or("") != "messages.google.com" {
                return;
            }
            let cookies = match webview.cookies_for_url(url.clone()) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("cookies_for_url failed: {e}");
                    return;
                }
            };
            // Wait until the SAPISID cookie is present — that's the
            // signal Google's login dance has completed.
            if !cookies.iter().any(|c| c.name() == "SAPISID") {
                return;
            }

            {
                let mut trig = triggered_cb.lock().unwrap();
                if *trig {
                    return;
                }
                *trig = true;
            }

            let cookie_map: HashMap<String, String> = cookies
                .into_iter()
                .map(|c| (c.name().to_string(), c.value().to_string()))
                .collect();

            let _ = webview.close();

            let app = app_cb.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let params = json!({ "cookies": cookie_map });
                if let Err(e) = state.bridge.call("pair_gaia", Some(params)).await {
                    log::error!("pair_gaia call failed: {e}");
                }
            });
        })
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Resolve the bridge binary path. In dev we use the in-tree build; in
/// release we'll switch to Tauri's sidecar resource path.
fn resolve_bridge_binary() -> PathBuf {
    if let Ok(p) = std::env::var("LUMINA_BRIDGE_BIN") {
        return PathBuf::from(p);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("bridge")
        .join("bin")
        .join("lumina-bridge")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("lumina-media", |ctx, request, responder| {
            // URL form: lumina-media://localhost/<media-id>. Tauri/WebKit
            // routes both `lumina-media://abc` and `lumina-media://localhost/abc`
            // here; we accept either by stripping a leading `/`.
            let path = request.uri().path();
            let media_id = path.strip_prefix('/').unwrap_or(path).to_string();
            let app = ctx.app_handle().clone();
            log::info!(
                "lumina-media request: uri={} path={} media_id={}",
                request.uri(),
                path,
                media_id
            );

            tauri::async_runtime::spawn(async move {
                let respond = move |status: u16, content_type: &str, body: Vec<u8>| {
                    let resp = tauri::http::Response::builder()
                        .status(status)
                        .header(tauri::http::header::CONTENT_TYPE, content_type)
                        // Decrypted bytes are immutable per id; let the
                        // webview cache aggressively for the session.
                        .header(tauri::http::header::CACHE_CONTROL, "private, max-age=86400, immutable")
                        // Custom-protocol responses default to a different
                        // origin than the page; relax CORS so fetch() etc
                        // also work if we ever need them.
                        .header(tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                        .body(body)
                        .unwrap();
                    responder.respond(resp);
                };

                if media_id.is_empty() {
                    respond(400, "text/plain", b"missing media id".to_vec());
                    return;
                }

                let state = app.state::<AppState>();
                let result = state
                    .bridge
                    .call("fetch_media", Some(json!({ "media_id": &media_id })))
                    .await;
                match result {
                    Ok(v) => {
                        let mime = v
                            .get("mime")
                            .and_then(Value::as_str)
                            .unwrap_or("application/octet-stream")
                            .to_string();
                        let b64 = match v.get("bytes_b64").and_then(Value::as_str) {
                            Some(s) => s,
                            None => {
                                log::warn!("lumina-media: bridge response missing bytes_b64 for {media_id}");
                                respond(502, "text/plain", b"bridge missing bytes_b64".to_vec());
                                return;
                            }
                        };
                        match base64::engine::general_purpose::STANDARD.decode(b64) {
                            Ok(bytes) => {
                                log::info!(
                                    "lumina-media ok: media_id={} mime={} bytes={}",
                                    media_id,
                                    mime,
                                    bytes.len()
                                );
                                respond(200, &mime, bytes);
                            }
                            Err(e) => {
                                log::warn!("lumina-media decode failed for {media_id}: {e}");
                                respond(
                                    502,
                                    "text/plain",
                                    format!("decode: {e}").into_bytes(),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("lumina-media bridge error for {media_id}: {e}");
                        respond(404, "text/plain", e.into_bytes());
                    }
                }
            });
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_local_data_dir()
                .expect("resolve app local data dir");
            let bin = resolve_bridge_binary();

            // Block on spawn so AppState is registered before the webview
            // is allowed to invoke any bridge_* command.
            let bridge = tauri::async_runtime::block_on(async {
                Bridge::spawn(app_handle.clone(), bin, data_dir).await
            })?;
            app.manage(AppState {
                bridge: Arc::new(bridge),
            });
            log::info!("lumina-bridge spawned");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bridge_status,
            bridge_pair,
            bridge_connect,
            bridge_unpair,
            bridge_list_conversations,
            bridge_fetch_messages,
            start_gaia_login,
            pair_with_cookies,
            detect_browsers,
            import_browser_cookies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
