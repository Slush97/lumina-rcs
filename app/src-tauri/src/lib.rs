mod bridge;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::bridge::Bridge;

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
            start_gaia_login,
            pair_with_cookies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
