use regex::Regex;
use std::io::Write;
use std::sync::OnceLock;

fn get_secret_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        vec![
            // Bearer tokens
            Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.\~+\/]+=*").unwrap(),
            // Basic authorization headers (base64)
            Regex::new(r"(?i)authorization:\s*basic\s+[a-zA-Z0-9_\-\.\~+\/]+=*").unwrap(),
            // Inline URI passwords: //user:password@host
            Regex::new(r"//[^:/]+:[^@/]+@").unwrap(),
            // Cookie headers
            Regex::new(r"(?i)cookie:\s*[^\n]+").unwrap(),
            // General secret/token key=value or key:value in JSON logs
            Regex::new(
                r"(?i)(rpc-secret|rpc_secret|password|secret|token|api[_-]?key|access[_-]?key)\s*=\s*[a-zA-Z0-9_\-\.\~+]+",
            )
            .unwrap(),
            Regex::new(r#"(?i)"(rpc-secret|rpc_secret|password|secret|token|api[_-]?key|access[_-]?key)"\s*:\s*"[^"]+""#)
                .unwrap(),
            // .netrc password entries: "password somevalue"
            Regex::new(r"(?i)\bpassword\s+[^\s]+").unwrap(),
            // PEM private key blocks
            Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----")
                .unwrap(),
            // URL-encoded passwords: %3A = ':', detect common encoded credential patterns
            Regex::new(r"(?i)(password|secret|token)%3[Aa][^&\s]+").unwrap(),
            // AWS-style secret access keys (40 char base62 after keyword)
            Regex::new(r"(?i)(aws_secret_access_key|aws_session_token)\s*[=:]\s*[a-zA-Z0-9/+]{20,}").unwrap(),
        ]
    })
}

pub struct ScrubbingWriter<W: Write> {
    inner: W,
}

impl<W: Write> ScrubbingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> Write for ScrubbingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = match std::str::from_utf8(buf) {
            Ok(t) => t,
            Err(_) => return self.inner.write(buf), // fallback for raw binary
        };

        let mut scrubbed = text.to_string();
        for re in get_secret_regexes().iter() {
            scrubbed = re
                .replace_all(&scrubbed, |caps: &regex::Captures| {
                    if let Some(matched) = caps.get(0) {
                        let s = matched.as_str();
                        if s.starts_with("//") {
                            if let Some(colon_idx) = s.find(':') {
                                format!("{}:[REDACTED]@", &s[..colon_idx])
                            } else {
                                "//[REDACTED]@".to_string()
                            }
                        } else if s.to_lowercase().contains("bearer") {
                            "Bearer [REDACTED]".to_string()
                        } else if s.to_lowercase().contains("basic") {
                            "Authorization: Basic [REDACTED]".to_string()
                        } else if s.to_lowercase().contains("cookie") {
                            "Cookie: [REDACTED]".to_string()
                        } else if s.contains("PRIVATE KEY") {
                            "-----BEGIN [REDACTED] PRIVATE KEY-----\n[REDACTED]\n-----END [REDACTED] PRIVATE KEY-----".to_string()
                        } else if s.contains(':') {
                            if let Some(key) = caps.get(1) {
                                format!("\"{}\":\"[REDACTED]\"", key.as_str())
                            } else {
                                "[REDACTED]".to_string()
                            }
                        } else if s.contains('=') {
                            if let Some(key) = caps.get(1) {
                                format!("{}=[REDACTED]", key.as_str())
                            } else {
                                "[REDACTED]".to_string()
                            }
                        } else {
                            "[REDACTED]".to_string()
                        }
                    } else {
                        "[REDACTED]".to_string()
                    }
                })
                .to_string();
        }

        self.inner.write_all(scrubbed.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub struct ScrubbingMakeWriter<M> {
    inner: M,
}

impl<M> ScrubbingMakeWriter<M> {
    pub fn new(inner: M) -> Self {
        Self { inner }
    }
}

impl<'a, M> tracing_subscriber::fmt::writer::MakeWriter<'a> for ScrubbingMakeWriter<M>
where
    M: tracing_subscriber::fmt::writer::MakeWriter<'a>,
{
    type Writer = ScrubbingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        ScrubbingWriter::new(self.inner.make_writer())
    }
}

#[cfg(test)]
#[path = "scrubber_tests.rs"]
mod tests;
