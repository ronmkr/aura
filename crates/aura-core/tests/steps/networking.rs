use crate::AuraWorld;
use cucumber::{given, then, when};

#[given(expr = "a global SOCKS5 proxy is configured at {string}")]
async fn given_global_proxy(_world: &mut AuraWorld, _proxy: String) {}

#[when(expr = "I add a BitTorrent task")]
async fn when_add_bittorrent_task(_world: &mut AuraWorld) {}

#[then(expr = "the {string} should establish all peer connections via the proxy")]
async fn then_establish_connections_via_proxy(_world: &mut AuraWorld, _worker: String) {}

#[then(expr = "the Tracker {string} request should include the proxy credentials")]
async fn then_tracker_request_proxy_credentials(_world: &mut AuraWorld, _request: String) {}

#[given(expr = "the engine is behind a UPnP-capable router")]
async fn given_upnp_router(_world: &mut AuraWorld) {}

#[when(expr = "the {string} starts")]
async fn when_actor_starts(_world: &mut AuraWorld, _actor: String) {}

#[then(expr = "it should request a port mapping for the {string} \\({int})")]
async fn then_request_port_mapping(_world: &mut AuraWorld, _port_name: String, _port: u32) {}

#[then(expr = "it should periodically refresh the mapping before it expires")]
async fn then_refresh_mapping(_world: &mut AuraWorld) {}

#[then(expr = "if UPnP fails, it should fallback to NAT-PMP")]
async fn then_fallback_to_nat_pmp(_world: &mut AuraWorld) {}

#[given(expr = "a mirror that supports both IPv4 and IPv6")]
async fn given_dual_stack_mirror(_world: &mut AuraWorld) {}

#[when(expr = "a {string} initiates a connection")]
async fn when_worker_initiates_connection(_world: &mut AuraWorld, _worker: String) {}

#[then(expr = "it should attempt to connect to both addresses in parallel")]
async fn then_connect_both_parallel(_world: &mut AuraWorld) {}

#[then(expr = "it should use the first one that successfully completes the handshake")]
async fn then_use_first_successful(_world: &mut AuraWorld) {}

#[then(expr = "it should cancel the lagging attempt immediately")]
async fn then_cancel_lagging_attempt(_world: &mut AuraWorld) {}
