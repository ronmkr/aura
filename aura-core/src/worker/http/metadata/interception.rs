use super::super::crawler::RecursiveCrawler;
use crate::{Error, Result};

pub(crate) async fn handle_html_interception(
    uri: &str,
    current_uri: &str,
    response: reqwest::Response,
) -> Result<String> {
    let body = response
        .text()
        .await
        .map_err(|e| Error::Protocol(format!("Failed to read HTML body: {}", e)))?;

    let asset_exts = [
        ".zip", ".tar.gz", ".tgz", ".dmg", ".exe", ".pkg", ".iso", ".rar", ".7z", ".bin", ".msi",
        ".pdf", ".mp4", ".mkv", ".tar",
    ];

    // --- CAPTIVE PORTAL INTERCEPTION ---
    let ends_with_asset = asset_exts
        .iter()
        .any(|ext| uri.to_lowercase().ends_with(ext));
    if ends_with_asset {
        let body_lower = body.to_lowercase();
        let keywords = [
            "login",
            "signin",
            "captive",
            "wifi",
            "portal",
            "hotspot",
            "accept terms",
            "gateway",
        ];
        let has_captive_keyword = keywords.iter().any(|&kw| body_lower.contains(kw));
        let url_lower = current_uri.to_lowercase();
        let has_captive_url = url_lower.contains("login")
            || url_lower.contains("portal")
            || url_lower.contains("wifi")
            || url_lower.contains("captive");

        if has_captive_keyword || has_captive_url {
            return Err(Error::CaptivePortal(format!(
                "Captive portal landing page detected at {}",
                current_uri
            )));
        }
    }
    // ------------------------------------

    if let Ok(mut crawler) = RecursiveCrawler::new(current_uri, 1, true) {
        crawler.enqueue_links(current_uri, &body, 0);
        let mut found = None;
        while let Some((link_url, _depth)) = crawler.next_url() {
            let path = link_url.to_lowercase();
            if asset_exts.iter().any(|ext| path.ends_with(ext)) {
                found = Some(link_url);
                break;
            }
        }
        if let Some(link) = found {
            tracing::info!(from = %current_uri, to = %link, "Resolved landing page direct link via crawler");
            return Ok(link);
        }
    }

    Err(Error::Protocol(format!(
        "URI {} points to an HTML landing page. Direct link resolution failed.",
        current_uri
    )))
}
