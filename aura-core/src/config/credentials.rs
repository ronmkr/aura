//! credentials: Unified provider for .netrc and cookie-based authentication.

use crate::{Error, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

/// Represents a set of credentials for a specific machine/host.
#[derive(Debug, Clone, Default)]
pub struct Credentials {
    pub login: Option<String>,
    pub password: Option<String>,
    pub account: Option<String>,
}

/// A centralized resolver for authentication data.
pub struct CredentialProvider {
    netrc: HashMap<String, Credentials>,
    cookie_jar: Arc<reqwest::cookie::Jar>,
}

impl CredentialProvider {
    /// Creates a new, empty CredentialProvider.
    pub fn new() -> Self {
        Self {
            netrc: HashMap::new(),
            cookie_jar: Arc::new(reqwest::cookie::Jar::default()),
        }
    }

    /// Returns a reference to the internal cookie jar.
    pub fn cookie_jar(&self) -> Arc<reqwest::cookie::Jar> {
        self.cookie_jar.clone()
    }

    /// Loads cookies from a file in Netscape format.
    pub fn load_cookies<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::open(path)
            .map_err(|e| Error::Config(format!("Failed to open cookie file: {}", e)))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line =
                line.map_err(|e| Error::Config(format!("Failed to read cookie file: {}", e)))?;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 7 {
                continue;
            }

            // Netscape format: domain, flag, path, secure, expiration, name, value
            let domain = parts[0];
            // let _flag = parts[1];
            let path_str = parts[2];
            let secure = parts[3].to_lowercase() == "true";
            // let _expiration = parts[4];
            let name = parts[5];
            let value = parts[6];

            let protocol = if secure { "https" } else { "http" };
            let url_str = format!("{}://{}", protocol, domain.trim_start_matches('.'));
            if let Ok(url) = url::Url::parse(&url_str) {
                let cookie_str =
                    format!("{}={}; Path={}; Domain={}", name, value, path_str, domain);
                self.cookie_jar.add_cookie_str(&cookie_str, &url);
            }
        }

        Ok(())
    }

    /// Loads credentials from a .netrc file.
    pub fn load_netrc<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let file =
            File::open(path).map_err(|e| Error::Config(format!("Failed to open .netrc: {}", e)))?;
        let reader = BufReader::new(file);

        let mut current_machine = String::new();
        let mut credentials = Credentials::default();

        for line in reader.lines() {
            let line = line.map_err(|e| Error::Config(format!("Failed to read .netrc: {}", e)))?;
            let tokens: Vec<&str> = line.split_whitespace().collect();

            let mut i = 0;
            while i < tokens.len() {
                match tokens[i] {
                    "machine" if i + 1 < tokens.len() => {
                        if !current_machine.is_empty() {
                            self.netrc.insert(current_machine, credentials);
                        }
                        current_machine = tokens[i + 1].to_string();
                        credentials = Credentials::default();
                        i += 2;
                    }
                    "login" if i + 1 < tokens.len() => {
                        credentials.login = Some(tokens[i + 1].to_string());
                        i += 2;
                    }
                    "password" if i + 1 < tokens.len() => {
                        credentials.password = Some(tokens[i + 1].to_string());
                        i += 2;
                    }
                    "account" if i + 1 < tokens.len() => {
                        credentials.account = Some(tokens[i + 1].to_string());
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
        }

        if !current_machine.is_empty() {
            self.netrc.insert(current_machine, credentials);
        }

        Ok(())
    }

    /// Resolves credentials for a given host.
    pub fn get_credentials(&self, host: &str) -> Option<&Credentials> {
        self.netrc.get(host)
    }
}

impl Default for CredentialProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_netrc_parsing() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "machine github.com login myuser password mypass\nmachine example.com login otheruser password otherpass account myacc"
        )
        .unwrap();

        let mut provider = CredentialProvider::new();
        provider.load_netrc(file.path()).unwrap();

        let creds = provider.get_credentials("github.com").unwrap();
        assert_eq!(creds.login.as_deref(), Some("myuser"));
        assert_eq!(creds.password.as_deref(), Some("mypass"));

        let creds2 = provider.get_credentials("example.com").unwrap();
        assert_eq!(creds2.login.as_deref(), Some("otheruser"));
        assert_eq!(creds2.account.as_deref(), Some("myacc"));
    }
}
