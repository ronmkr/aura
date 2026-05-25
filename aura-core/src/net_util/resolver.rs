//! resolver: DNS-over-HTTPS (DoH) and DNS-over-TLS (DoT) resolver construction and bootstrap logic.

use crate::config::{ResolverConfig, StructuredResolverConfig};
use crate::{Error, Result};
pub use hickory_resolver::TokioResolver;
use std::net::IpAddr;
use std::sync::Arc;

/// Creates a `TokioResolver` from the provided custom `ResolverConfig`.
pub async fn create_resolver(config: &ResolverConfig) -> Result<TokioResolver> {
    use hickory_resolver::proto::xfer::Protocol;
    match config {
        ResolverConfig::Simple(ref s) => {
            let s_lower = s.to_lowercase();
            if s_lower == "system" {
                let builder = TokioResolver::builder_tokio()
                    .map_err(|e| {
                        Error::Config(format!("Failed to init system DNS builder: {}", e))
                    })?;
                Ok(builder.build())
            } else if s_lower == "cloudflare" {
                Ok(hickory_resolver::Resolver::builder_with_config(
                    hickory_resolver::config::ResolverConfig::cloudflare_https(),
                    hickory_resolver::name_server::TokioConnectionProvider::default(),
                )
                .build())
            } else if s_lower == "google" {
                Ok(hickory_resolver::Resolver::builder_with_config(
                    hickory_resolver::config::ResolverConfig::google_https(),
                    hickory_resolver::name_server::TokioConnectionProvider::default(),
                )
                .build())
            } else {
                let ip: IpAddr = s.parse().map_err(|_| {
                    Error::Config(format!("Unsupported or invalid dns_resolver config: {}", s))
                })?;
                let mut hickory_config = hickory_resolver::config::ResolverConfig::new();
                let addr = std::net::SocketAddr::new(ip, 53);
                hickory_config.add_name_server(hickory_resolver::config::NameServerConfig::new(
                    addr,
                    Protocol::Udp,
                ));

                Ok(hickory_resolver::Resolver::builder_with_config(
                    hickory_config,
                    hickory_resolver::name_server::TokioConnectionProvider::default(),
                )
                .build())
            }
        }
        ResolverConfig::Structured(ref structured) => match structured {
            StructuredResolverConfig::Doh { url, ips } => {
                let parsed_url = url::Url::parse(url)
                    .map_err(|e| Error::Config(format!("Invalid DNS-over-HTTPS URL: {}", e)))?;
                let host = parsed_url
                    .host_str()
                    .ok_or_else(|| {
                        Error::Config(format!("DNS-over-HTTPS URL must have a host: {}", url))
                    })?
                    .to_string();
                let port = parsed_url.port().unwrap_or(443);

                let resolved_ips = match ips {
                    Some(ips) if !ips.is_empty() => {
                        let mut parsed_ips = Vec::new();
                        for ip_str in ips {
                            let ip: IpAddr = ip_str.parse().map_err(|_| {
                                Error::Config(format!("Invalid DoH IP address: {}", ip_str))
                            })?;
                            parsed_ips.push(ip);
                        }
                        parsed_ips
                    }
                    _ => {
                        let addrs = tokio::net::lookup_host(format!("{}:{}", host, port))
                            .await
                            .map_err(|e| {
                                Error::Config(format!(
                                    "Failed to bootstrap DoH host resolution for {}: {}",
                                    host, e
                                ))
                            })?;
                        addrs.map(|addr| addr.ip()).collect::<Vec<_>>()
                    }
                };

                if resolved_ips.is_empty() {
                    return Err(Error::Config(format!(
                        "No IP addresses found to bootstrap DoH resolver for host {}",
                        host
                    )));
                }

                let mut hickory_config = hickory_resolver::config::ResolverConfig::new();
                let server_name = host.clone();

                for ip in resolved_ips {
                    let mut ns_config = hickory_resolver::config::NameServerConfig::new(
                        std::net::SocketAddr::new(ip, port),
                        Protocol::Https,
                    );
                    ns_config.tls_dns_name = Some(server_name.clone());
                    ns_config.http_endpoint = Some(parsed_url.path().to_string());
                    hickory_config.add_name_server(ns_config);
                }

                Ok(hickory_resolver::Resolver::builder_with_config(
                    hickory_config,
                    hickory_resolver::name_server::TokioConnectionProvider::default(),
                )
                .build())
            }
            StructuredResolverConfig::Dot {
                server,
                port,
                tls_name,
            } => {
                let resolved_port = port.unwrap_or(853);
                let resolved_ips = if let Ok(ip) = server.parse::<IpAddr>() {
                    vec![ip]
                } else {
                    let addrs = tokio::net::lookup_host(format!("{}:{}", server, resolved_port))
                        .await
                        .map_err(|e| {
                            Error::Config(format!(
                                "Failed to resolve DoT server hostname {}: {}",
                                server, e
                            ))
                        })?;
                    addrs.map(|addr| addr.ip()).collect::<Vec<_>>()
                };

                if resolved_ips.is_empty() {
                    return Err(Error::Config(format!(
                        "No IP addresses resolved for DoT server {}",
                        server
                    )));
                }

                let mut hickory_config = hickory_resolver::config::ResolverConfig::new();

                for ip in resolved_ips {
                    let mut ns_config = hickory_resolver::config::NameServerConfig::new(
                        std::net::SocketAddr::new(ip, resolved_port),
                        Protocol::Tls,
                    );
                    ns_config.tls_dns_name = Some(tls_name.clone());
                    hickory_config.add_name_server(ns_config);
                }

                Ok(hickory_resolver::Resolver::builder_with_config(
                    hickory_config,
                    hickory_resolver::name_server::TokioConnectionProvider::default(),
                )
                .build())
            }
        },
    }
}

/// A wrapper around `TokioResolver` that implements `reqwest::dns::Resolve`.
#[derive(Debug, Clone)]
pub struct ReqwestDnsResolver {
    inner: Arc<TokioResolver>,
}

impl ReqwestDnsResolver {
    pub fn from_arc(resolver: Arc<TokioResolver>) -> Self {
        Self { inner: resolver }
    }
}

impl reqwest::dns::Resolve for ReqwestDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolver = self.inner.clone();
        let name_str = name.as_str().to_string();
        Box::pin(async move {
            let lookup = resolver
                .lookup_ip(name_str)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let addrs: Box<dyn Iterator<Item = std::net::SocketAddr> + Send> = Box::new(
                lookup
                    .iter()
                    .map(|ip| std::net::SocketAddr::new(ip, 0))
                    .collect::<Vec<_>>()
                    .into_iter(),
            );
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_resolver_simple_system() {
        let config = ResolverConfig::Simple("system".to_string());
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create system resolver: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_simple_cloudflare() {
        let config = ResolverConfig::Simple("cloudflare".to_string());
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create cloudflare resolver: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_simple_google() {
        let config = ResolverConfig::Simple("google".to_string());
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create google resolver: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_simple_custom_ip() {
        let config = ResolverConfig::Simple("127.0.0.1".to_string());
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create custom IP resolver: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_doh_with_ips() {
        let config = ResolverConfig::Structured(StructuredResolverConfig::Doh {
            url: "https://cloudflare-dns.com/dns-query".to_string(),
            ips: Some(vec!["1.1.1.1".to_string()]),
        });
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create DoH resolver with IPs: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_doh_bootstrap() {
        let config = ResolverConfig::Structured(StructuredResolverConfig::Doh {
            url: "https://localhost/dns-query".to_string(),
            ips: None,
        });
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create DoH resolver with bootstrap: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_dot_with_ip() {
        let config = ResolverConfig::Structured(StructuredResolverConfig::Dot {
            server: "127.0.0.1".to_string(),
            port: Some(853),
            tls_name: "localhost".to_string(),
        });
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create DoT resolver with IP: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_resolver_dot_bootstrap() {
        let config = ResolverConfig::Structured(StructuredResolverConfig::Dot {
            server: "localhost".to_string(),
            port: Some(853),
            tls_name: "localhost".to_string(),
        });
        let result = create_resolver(&config).await;
        assert!(
            result.is_ok(),
            "Failed to create DoT resolver with bootstrap: {:?}",
            result.err()
        );
    }
}
