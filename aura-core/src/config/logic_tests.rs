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
