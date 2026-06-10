use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VpnConfig {
    pub type_name: Option<String>, // "openvpn", "wireguard"
    pub profile_path: Option<String>,
    pub management_addr: Option<String>,
    pub auto_connect: bool,
    pub check_interval_secs: u64,
    pub connect_timeout_secs: u64,
    pub force_tunnel: bool,
}

impl Default for VpnConfig {
    fn default() -> Self {
        Self {
            type_name: None,
            profile_path: None,
            management_addr: None,
            auto_connect: false,
            check_interval_secs: 5,
            connect_timeout_secs: 5,
            force_tunnel: false,
        }
    }
}
