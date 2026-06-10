/// Parses the filename from a `Content-Disposition` header value, following
/// RFC 6266 §4.3 (prefer `filename*` over `filename`) and RFC 5987 encoding.
///
/// Security guarantees:
/// - Strips all path separators (`/`, `\\`, `:`) to prevent path traversal.
/// - Strips null bytes and ASCII control characters (0x00–0x1F, 0x7F).
/// - Truncates to 255 bytes (POSIX NAME_MAX).
/// - Returns `None` if the result is empty after sanitization.
pub(crate) fn parse_content_disposition(header: &str) -> Option<String> {
    // Prefer filename* (RFC 5987 encoded) over plain filename
    let starred = header.split(';').find_map(|part| {
        let part = part.trim();
        // filename*=UTF-8''encoded%20name
        let rest = part.strip_prefix_ci("filename*=")?;
        // Strip optional charset prefix: UTF-8'' or ISO-8859-1''
        let encoded = if let Some(pos) = rest.find("''") {
            &rest[pos + 2..]
        } else {
            rest
        };
        // Percent-decode manually — avoid pulling in an extra crate
        percent_decode(encoded)
    });

    let raw = if let Some(s) = starred {
        s
    } else {
        // Fall back to plain filename=
        header.split(';').find_map(|part| {
            let part = part.trim();
            let rest = part.strip_prefix_ci("filename=")?;
            Some(rest.trim_matches('"').to_string())
        })?
    };

    sanitize_filename(&raw)
}

/// Case-insensitive prefix strip helper (not yet stable in std as a method).
trait StripPrefixCi {
    fn strip_prefix_ci(&self, prefix: &str) -> Option<&str>;
}
impl StripPrefixCi for str {
    fn strip_prefix_ci(&self, prefix: &str) -> Option<&str> {
        if self.len() >= prefix.len() && self[..prefix.len()].eq_ignore_ascii_case(prefix) {
            Some(&self[prefix.len()..])
        } else {
            None
        }
    }
}

/// Percent-decodes a string like `report%20final.pdf` → `report final.pdf`.
fn percent_decode(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = char::from(bytes[i + 1]).to_digit(16)?;
            let lo = char::from(bytes[i + 2]).to_digit(16)?;
            let byte = ((hi << 4) | lo) as u8;
            out.push(byte as char);
            i += 3;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    Some(out)
}

/// Sanitizes a raw filename string:
/// - Removes path separators and null/control bytes
/// - Truncates to 255 bytes
/// - Returns `None` if result is empty
fn sanitize_filename(raw: &str) -> Option<String> {
    let sanitized: String = raw
        .chars()
        .filter(|&c| {
            c != '/' && c != '\\' && c != ':' // path separators
                && c != '\0'                  // null byte
                && !c.is_control() // ASCII control chars
        })
        .collect();

    // Truncate to POSIX NAME_MAX (255 bytes)
    let truncated = if sanitized.len() > 255 {
        // Find a valid UTF-8 boundary at or before byte 255
        let mut boundary = 255;
        while !sanitized.is_char_boundary(boundary) {
            boundary -= 1;
        }
        sanitized[..boundary].to_string()
    } else {
        sanitized
    };

    if truncated.is_empty() {
        None
    } else {
        Some(truncated)
    }
}
