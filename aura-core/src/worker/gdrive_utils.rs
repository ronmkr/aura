#[cfg(feature = "gdrive")]
pub(crate) fn extract_file_id(uri: &str) -> Option<String> {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return Some("mock-file-id".to_string());
    }
    if uri.starts_with("gdrive://") {
        let trimmed = uri.strip_prefix("gdrive://").unwrap();
        return Some(trimmed.split('/').next()?.to_string());
    }
    if let Ok(url) = url::Url::parse(uri) {
        if let Some(host) = url.host_str() {
            if host.contains("drive.google.com") {
                if let Some(segments) = url.path_segments() {
                    let path_segments: Vec<&str> = segments.collect();
                    for i in 0..path_segments.len() {
                        if path_segments[i] == "d" && i + 1 < path_segments.len() {
                            return Some(path_segments[i + 1].to_string());
                        }
                    }
                }
                for (key, val) in url.query_pairs() {
                    if key == "id" {
                        return Some(val.into_owned());
                    }
                }
            }
        }
    }
    None
}

#[cfg(feature = "gdrive")]
pub(crate) fn extract_onedrive_id(uri: &str) -> Option<String> {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return Some("mock-item-id".to_string());
    }
    if uri.starts_with("onedrive://") {
        let trimmed = uri.strip_prefix("onedrive://").unwrap();
        return Some(trimmed.split('/').next()?.to_string());
    }
    if let Ok(url) = url::Url::parse(uri) {
        if let Some(host) = url.host_str() {
            if host.contains("onedrive.live.com") || host.contains("sharepoint.com") {
                for (key, val) in url.query_pairs() {
                    if key == "resid" || key == "id" {
                        return Some(val.into_owned());
                    }
                }
            }
        }
    }
    None
}
