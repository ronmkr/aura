use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub interface: Option<String>,
    pub local_addr: Option<std::net::IpAddr>,
    pub bind_address: std::net::IpAddr,
    pub listen_port: u16,
    pub dht_port: u16,
    pub rpc_port: u16,
    pub rpc_secret: Option<String>,
    pub allowed_origins: Vec<String>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub user_agent: String,
    pub connect_timeout_secs: u64,
    pub tcp_keepalive_secs: u64,
    pub proxy: Option<String>,
    pub max_redirects: usize,
    pub http_retry_count: u32,
    pub http_retry_delay_secs: u64,
    pub happy_eyeballs_stagger_ms: u64,
    pub http_buffer_capacity: usize,
    pub http_concurrent_requests: usize,
    pub dns_resolver: ResolverConfig,
    pub nat_refresh_interval_secs: u64,
    pub tracker_timeout_secs: u64,
    pub udp_tracker_timeout_secs: u64,
    pub roaming_reconnect_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ResolverConfig {
    Simple(String),
    Structured(StructuredResolverConfig),
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self::Simple("system".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StructuredResolverConfig {
    Doh {
        url: String,
        ips: Option<Vec<String>>,
    },
    Dot {
        server: String,
        port: Option<u16>,
        tls_name: String,
    },
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            interface: None,
            local_addr: None,
            bind_address: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            listen_port: 6881,
            dht_port: 6881,
            rpc_port: 6800,
            rpc_secret: None,
            allowed_origins: vec![
                "http://localhost".to_string(),
                "http://127.0.0.1".to_string(),
                "chrome-extension://".to_string(),
            ],
            tls_cert: None,
            tls_key: None,
            user_agent: "Aura/0.1.0".to_string(),
            connect_timeout_secs: 30,
            tcp_keepalive_secs: 60,
            proxy: None,
            max_redirects: 20,
            http_retry_count: 5,
            http_retry_delay_secs: 2,
            happy_eyeballs_stagger_ms: 250,
            http_buffer_capacity: 16384,
            http_concurrent_requests: 32,
            dns_resolver: ResolverConfig::default(),
            nat_refresh_interval_secs: 1800,
            tracker_timeout_secs: 10,
            udp_tracker_timeout_secs: 5,
            roaming_reconnect_delay_ms: 500,
        }
    }
}
