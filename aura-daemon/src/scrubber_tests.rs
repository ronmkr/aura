use super::*;

#[test]
fn test_scrubbing_writer_redaction() {
    let inputs = vec![
        (
            "Connecting with Bearer s3cr3t_t0k3n now",
            "Connecting with Bearer [REDACTED] now",
        ),
        (
            "Authorization: Basic dXNlcjpwYXNz",
            "Authorization: Basic [REDACTED]",
        ),
        ("Cookie: session_id=12345; auth=true", "Cookie: [REDACTED]"),
        (
            "Fetching url: http://user:supersecurepass@example.com/file",
            "Fetching url: http://user:[REDACTED]@example.com/file",
        ),
        (
            "Set rpc-secret=aura_secret_token default",
            "Set rpc-secret=[REDACTED] default",
        ),
        (
            "Config: \"rpc_secret\":\"mysecret\", \"port\":6800",
            "Config: \"rpc_secret\":\"[REDACTED]\", \"port\":6800",
        ),
    ];

    for (input, expected) in inputs {
        let mut buf = Vec::new();
        let mut writer = ScrubbingWriter::new(&mut buf);
        writer.write_all(input.as_bytes()).unwrap();
        writer.flush().unwrap();
        let result = String::from_utf8(buf).unwrap();
        assert_eq!(result, expected, "Failed for input: {}", input);
    }
}
