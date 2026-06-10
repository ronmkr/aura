use super::*;

#[test]
fn test_deserialize_dns_resolver_simple() {
    let toml_str = r#"
        [network]
        dns_resolver = "cloudflare"
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(
        config.network.dns_resolver,
        ResolverConfig::Simple("cloudflare".to_string())
    );

    let toml_str = r#"
        [network]
        dns_resolver = "system"
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(
        config.network.dns_resolver,
        ResolverConfig::Simple("system".to_string())
    );
}

#[test]
fn test_deserialize_dns_resolver_doh() {
    let toml_str = r#"
        [network.dns_resolver]
        type = "doh"
        url = "https://cloudflare-dns.com/dns-query"
        ips = ["1.1.1.1", "1.0.0.1"]
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(
        config.network.dns_resolver,
        ResolverConfig::Structured(StructuredResolverConfig::Doh {
            url: "https://cloudflare-dns.com/dns-query".to_string(),
            ips: Some(vec!["1.1.1.1".to_string(), "1.0.0.1".to_string()]),
        })
    );
}

#[test]
fn test_deserialize_dns_resolver_dot() {
    let toml_str = r#"
        [network.dns_resolver]
        type = "dot"
        server = "1.1.1.1"
        port = 853
        tls_name = "cloudflare-dns.com"
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(
        config.network.dns_resolver,
        ResolverConfig::Structured(StructuredResolverConfig::Dot {
            server: "1.1.1.1".to_string(),
            port: Some(853),
            tls_name: "cloudflare-dns.com".to_string(),
        })
    );
}

#[test]
fn test_empty_config_deserialization() {
    let toml_str = "";
    let config: Config = toml::from_str(toml_str).unwrap();

    // Check that defaults are fully filled
    assert_eq!(config.network.listen_port, 6881);
    assert_eq!(config.bandwidth.global_download_limit, 0);
    assert_eq!(config.bandwidth.max_concurrent_downloads, 10);
    assert_eq!(config.bittorrent.max_peers_per_torrent, 200);
    assert_eq!(config.storage.download_dir, ".".to_string());
    assert!(!config.vpn.auto_connect);
    assert_eq!(config.general.log_level, "info".to_string());
    assert_eq!(config.general.theme.primary, "#0000FF");
}

#[test]
fn test_partial_config_deserialization() {
    let toml_str = r#"
        [network]
        listen_port = 9000
        
        [bandwidth]
        global_download_limit = 500000
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();

    // Overridden fields
    assert_eq!(config.network.listen_port, 9000);
    assert_eq!(config.bandwidth.global_download_limit, 500000);

    // Default fallback fields
    assert_eq!(config.network.dht_port, 6881);
    assert_eq!(config.bandwidth.max_concurrent_downloads, 10);
}

#[test]
fn test_invalid_toml_syntax() {
    let toml_str = r#"
        [network
        listen_port = 9000
    "#;
    let result = toml::from_str::<Config>(toml_str);
    assert!(result.is_err());
}

#[test]
fn test_invalid_dns_resolver_type() {
    let toml_str = r#"
        [network.dns_resolver]
        type = "invalid_type"
        server = "1.1.1.1"
    "#;
    let result = toml::from_str::<Config>(toml_str);
    assert!(result.is_err());
}

#[test]
fn test_apply_cli_overrides() {
    let mut config = Config::default();
    config.apply_cli_overrides(CliOverrides {
        download_dir: Some("custom_dir".to_string()),
        limit: Some(12345),
        proxy: Some("http://proxy.com".to_string()),
        bind_address: Some("127.0.0.1".to_string()),
        rpc_port: Some(9999),
        rpc_secret: Some("token123".to_string()),
        tls_cert: Some("cert_path".to_string()),
        tls_key: Some("key_path".to_string()),
    });

    assert_eq!(config.storage.download_dir, "custom_dir");
    assert_eq!(config.bandwidth.global_download_limit, 12345);
    assert_eq!(config.network.proxy, Some("http://proxy.com".to_string()));
    assert_eq!(
        config.network.bind_address,
        "127.0.0.1".parse::<std::net::IpAddr>().unwrap()
    );
    assert_eq!(config.network.rpc_port, 9999);
    assert_eq!(config.network.rpc_secret, Some("token123".to_string()));
    assert_eq!(config.network.tls_cert, Some("cert_path".to_string()));
    assert_eq!(config.network.tls_key, Some("key_path".to_string()));
}

#[test]
fn test_load_resolved_custom_path() {
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new().unwrap();
    let toml_content = r#"
        [network]
        listen_port = 7777
    "#;
    temp.write_all(toml_content.as_bytes()).unwrap();

    let path_str = temp.path().to_str().unwrap();
    let config = Config::load_resolved(Some(path_str)).unwrap();
    assert_eq!(config.network.listen_port, 7777);
    assert_eq!(config.config_path, Some(temp.path().to_path_buf()));
}
