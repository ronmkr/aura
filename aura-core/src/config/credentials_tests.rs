use super::*;
use reqwest::cookie::CookieStore;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_netrc_parsing_advanced() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        r#"
        # This is a comment
        machine github.com # trailing comment
        login myuser
        password mypass

        default login guest password anon
        "#
    )
    .unwrap();

    let mut provider = CredentialProvider::new();
    provider.load_netrc(file.path()).unwrap();

    let creds = provider.get_credentials("github.com").unwrap();
    assert_eq!(creds.login.as_deref(), Some("myuser"));
    assert_eq!(creds.password.as_deref(), Some("mypass"));

    let creds2 = provider.get_credentials("unknown.com").unwrap();
    assert_eq!(creds2.login.as_deref(), Some("guest"));
    assert_eq!(creds2.password.as_deref(), Some("anon"));
}

#[test]
fn test_cookie_parsing_advanced() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        r#"# Netscape HTTP Cookie File
.google.com	TRUE	/	TRUE	2147483647	NID	val1
example.com	FALSE	/path	FALSE	2147483647	session	val2
"#
    )
    .unwrap();

    let provider = CredentialProvider::new();
    provider.load_cookies(file.path()).unwrap();

    let jar = provider.cookie_jar();

    let url1 = url::Url::parse("https://google.com").unwrap();
    let cookie1 = jar.cookies(&url1).unwrap();
    assert!(cookie1.to_str().unwrap().contains("NID=val1"));

    let url2 = url::Url::parse("http://example.com/path").unwrap();
    let cookie2 = jar.cookies(&url2).unwrap();
    assert!(cookie2.to_str().unwrap().contains("session=val2"));
}
