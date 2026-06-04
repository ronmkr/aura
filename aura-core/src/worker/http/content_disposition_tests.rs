use crate::worker::http::metadata::parse_content_disposition;

#[test]
fn test_plain_filename() {
    assert_eq!(
        parse_content_disposition("attachment; filename=\"report.pdf\""),
        Some("report.pdf".to_string())
    );
}

#[test]
fn test_filename_star_takes_priority() {
    assert_eq!(
        parse_content_disposition(
            "attachment; filename=\"wrong.pdf\"; filename*=UTF-8''correct%20file.pdf"
        ),
        Some("correct file.pdf".to_string())
    );
}

#[test]
fn test_path_traversal_stripped() {
    // sanitize_filename removes '/', '\', ':' only.
    // "../../evil.sh" → both '/' chars stripped → "....evil.sh"
    let result = parse_content_disposition("attachment; filename=\"../../evil.sh\"");
    assert_eq!(result, Some("....evil.sh".to_string()));
}

#[test]
fn test_null_byte_stripped() {
    let result = parse_content_disposition("attachment; filename=\"safe.pdf\0.exe\"");
    assert_eq!(result, Some("safe.pdf.exe".to_string()));
}

#[test]
fn test_empty_after_sanitization_returns_none() {
    let result = parse_content_disposition("attachment; filename=\"/\"");
    assert_eq!(result, None);
}

#[test]
fn test_truncated_at_255_bytes() {
    let long_name = "a".repeat(300);
    let header = format!("attachment; filename=\"{}\"", long_name);
    let result = parse_content_disposition(&header).unwrap();
    assert!(result.len() <= 255);
}
