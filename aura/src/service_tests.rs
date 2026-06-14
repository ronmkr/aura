use super::*;
use std::ffi::OsString;

#[test]
fn test_build_install_args_empty() {
    let args = build_install_args(None, None, None);

    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            args,
            vec![
                OsString::from("daemon"),
                OsString::from("--windows-service")
            ]
        );
    }
    #[cfg(not(target_os = "windows"))]
    {
        assert_eq!(args, vec![OsString::from("daemon")]);
    }
}

#[test]
fn test_build_install_args_all() {
    let args = build_install_args(
        Some("Aura.toml".to_string()),
        Some("127.0.0.1".to_string()),
        Some(6800),
    );

    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            args,
            vec![
                OsString::from("daemon"),
                OsString::from("--config"),
                OsString::from("Aura.toml"),
                OsString::from("--bind-address"),
                OsString::from("127.0.0.1"),
                OsString::from("--rpc-port"),
                OsString::from("6800"),
                OsString::from("--windows-service"),
            ]
        );
    }
    #[cfg(not(target_os = "windows"))]
    {
        assert_eq!(
            args,
            vec![
                OsString::from("daemon"),
                OsString::from("--config"),
                OsString::from("Aura.toml"),
                OsString::from("--bind-address"),
                OsString::from("127.0.0.1"),
                OsString::from("--rpc-port"),
                OsString::from("6800"),
            ]
        );
    }
}
