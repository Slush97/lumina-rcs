//! Browser cookie import via the `rookie` crate.
//!
//! Google blocks login from WebKitGTK, so users normally have to copy
//! Google session cookies in by hand. This module skips the paste step
//! by reading the cookies directly from a browser the user is already
//! signed into.

use std::collections::HashMap;

use serde::Serialize;

const GOOGLE_DOMAIN: &str = "google.com";

#[derive(Serialize, Clone, Debug)]
pub struct DetectedBrowser {
    pub id: String,
    pub display: String,
    pub cookie_count: usize,
    pub has_sapisid: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct ImportedCookies {
    pub browser: String,
    pub cookies: HashMap<String, String>,
}

/// Every browser rookie supports. The order here is the order they're
/// shown to the user, and the order `auto` tries them in.
fn extractor(id: &str) -> Option<fn(Option<Vec<String>>) -> rookie::Result<Vec<rookie::common::enums::Cookie>>> {
    match id {
        "brave" => Some(rookie::brave),
        "chrome" => Some(rookie::chrome),
        "chromium" => Some(rookie::chromium),
        "edge" => Some(rookie::edge),
        "vivaldi" => Some(rookie::vivaldi),
        "opera" => Some(rookie::opera),
        "opera_gx" => Some(rookie::opera_gx),
        "arc" => Some(rookie::arc),
        "firefox" => Some(rookie::firefox),
        "librewolf" => Some(rookie::librewolf),
        "zen" => Some(rookie::zen),
        #[cfg(target_os = "macos")]
        "safari" => Some(rookie::safari),
        #[cfg(target_os = "windows")]
        "internet_explorer" => Some(rookie::internet_explorer),
        _ => None,
    }
}

fn display_name(id: &str) -> &'static str {
    match id {
        "brave" => "Brave",
        "chrome" => "Google Chrome",
        "chromium" => "Chromium",
        "edge" => "Microsoft Edge",
        "vivaldi" => "Vivaldi",
        "opera" => "Opera",
        "opera_gx" => "Opera GX",
        "arc" => "Arc",
        "firefox" => "Firefox",
        "librewolf" => "LibreWolf",
        "zen" => "Zen",
        "safari" => "Safari",
        "internet_explorer" => "Internet Explorer",
        _ => "Unknown",
    }
}

/// Browsers we'll probe, in priority order. The first one with a usable
/// SAPISID wins for `auto` import.
const CANDIDATES: &[&str] = &[
    "brave",
    "chrome",
    "chromium",
    "vivaldi",
    "edge",
    "opera",
    "opera_gx",
    "arc",
    "firefox",
    "librewolf",
    "zen",
    #[cfg(target_os = "macos")]
    "safari",
];

fn extract(id: &str) -> Result<Vec<rookie::common::enums::Cookie>, String> {
    let f = extractor(id).ok_or_else(|| format!("unsupported browser: {id}"))?;
    f(Some(vec![GOOGLE_DOMAIN.to_string()])).map_err(|e| e.to_string())
}

/// Probe every supported browser and return the ones that yielded
/// at least one google.com cookie. Errors per-browser are swallowed —
/// most browsers won't be installed and that's fine.
pub fn detect() -> Vec<DetectedBrowser> {
    let mut found = Vec::new();
    for id in CANDIDATES {
        match extract(id) {
            Ok(cookies) if !cookies.is_empty() => {
                let has_sapisid = cookies.iter().any(|c| c.name == "SAPISID");
                found.push(DetectedBrowser {
                    id: (*id).to_string(),
                    display: display_name(id).to_string(),
                    cookie_count: cookies.len(),
                    has_sapisid,
                });
            }
            Ok(_) => {
                log::debug!("{id}: no google.com cookies");
            }
            Err(e) => {
                log::debug!("{id}: {e}");
            }
        }
    }
    found
}

/// Pull google.com cookies from a specific browser by id.
pub fn import(browser: &str) -> Result<ImportedCookies, String> {
    let cookies = extract(browser)?;
    if cookies.is_empty() {
        return Err(format!(
            "{} has no google.com cookies. Sign into messages.google.com/web in that browser first.",
            display_name(browser)
        ));
    }
    let map: HashMap<String, String> = cookies
        .into_iter()
        .map(|c| (c.name, c.value))
        .collect();
    if !map.contains_key("SAPISID") {
        return Err(format!(
            "{} has google.com cookies but no SAPISID. Make sure you're signed into messages.google.com/web in that browser.",
            display_name(browser)
        ));
    }
    Ok(ImportedCookies {
        browser: browser.to_string(),
        cookies: map,
    })
}
