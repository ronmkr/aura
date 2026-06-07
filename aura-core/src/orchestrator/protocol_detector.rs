use crate::task::TaskType;
use regex::Regex;
use std::path::Path;

/// Possible protocols detected by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedType {
    Http,
    Https,
    Ftp,
    Ftps,
    BitTorrent,
    Metalink,
}

impl DetectedType {
    pub fn to_task_type(&self) -> TaskType {
        match self {
            DetectedType::Http | DetectedType::Https => TaskType::Http,
            DetectedType::Ftp | DetectedType::Ftps => TaskType::Ftp,
            DetectedType::BitTorrent => TaskType::BitTorrent,
            DetectedType::Metalink => TaskType::Http, // Initially fetched as HTTP or read from file
        }
    }
}

pub struct ProtocolDetector;

impl ProtocolDetector {
    /// Detects the protocol type of the given input string.
    /// Input can be a URI, a local file path, or a BitTorrent Info-Hash.
    pub async fn detect(input: &str) -> Option<DetectedType> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        // 1. Check for well-known URI schemes
        if input.starts_with("magnet:") {
            return Some(DetectedType::BitTorrent);
        }
        if input.starts_with("http://") {
            return Some(DetectedType::Http);
        }
        if input.starts_with("https://") {
            return Some(DetectedType::Https);
        }
        if input.starts_with("ftp://") {
            return Some(DetectedType::Ftp);
        }
        if input.starts_with("ftps://") {
            return Some(DetectedType::Ftps);
        }

        // 2. Check for BitTorrent Info-Hashes (hex)
        // v1: 40 chars, v2: 64 chars
        let hex_re = Regex::new(r"^[0-9a-fA-F]{40}$|^[0-9a-fA-F]{64}$").unwrap();
        if hex_re.is_match(input) {
            return Some(DetectedType::BitTorrent);
        }

        // 3. Check for BitTorrent Info-Hashes (base32)
        // v1: 32 chars
        let b32_re = Regex::new(r"^[a-zA-Z2-7]{32}$").unwrap();
        if b32_re.is_match(input) {
            return Some(DetectedType::BitTorrent);
        }

        // 4. Check if it's a local file path and inspect its extension/content
        let path = Path::new(input);
        if path.exists() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext = ext.to_lowercase();
                if ext == "torrent" {
                    return Some(DetectedType::BitTorrent);
                }
                if ext == "metalink" || ext == "meta4" {
                    return Some(DetectedType::Metalink);
                }
            }

            // Peek at file content for bencode or XML
            if let Ok(mut file) = tokio::fs::File::open(path).await {
                use tokio::io::AsyncReadExt;
                let mut buffer = [0u8; 1024];
                if let Ok(n) = file.read(&mut buffer).await {
                    let content = &buffer[..n];

                    // BitTorrent bencoded dict usually starts with 'd'
                    if content.starts_with(b"d8:announce")
                        || content.starts_with(b"d13:announce-list")
                        || content.starts_with(b"d4:info")
                    {
                        return Some(DetectedType::BitTorrent);
                    }

                    // Metalink XML
                    let content_str = String::from_utf8_lossy(content);
                    if content_str.contains("<metalink") {
                        return Some(DetectedType::Metalink);
                    }
                }
            }
        }

        // 5. Last resort: extension-based detection for non-existent paths or URIs without scheme
        if input.ends_with(".torrent") {
            return Some(DetectedType::BitTorrent);
        }
        if input.ends_with(".metalink") || input.ends_with(".meta4") {
            return Some(DetectedType::Metalink);
        }

        None
    }
}

#[cfg(test)]
#[path = "protocol_detector_tests.rs"]
mod tests;
