# Configuration

Aura uses a central `Aura.toml` file for global and per-task configuration.

## Location

By default, Aura looks for `Aura.toml` in the current working directory. On startup, it will search standard OS config paths in the future.

## Key Settings

### Bandwidth
```toml
[bandwidth]
global_download_limit = 0 # 0 for unlimited
global_upload_limit = 1048576 # 1 MB/s
max_connections_per_task = 16
```

### Storage
```toml
[storage]
download_dir = "downloads/"
save_session_interval_secs = 60
```

### Privacy & Network
```toml
[network]
interface = "tun0"                     # Bind to a specific network interface (e.g., VPN tunnel)
local_addr = "192.168.1.100"         # Bind to a specific local IP address
listen_port = 6881                     # Port for incoming BitTorrent connections
dht_port = 6881                        # Port for DHT (UDP)
rpc_port = 6800                        # Port for JSON-RPC 2.0 server
rpc_secret = "your_secret_here"        # Optional secret for RPC authentication
user_agent = "Aura/0.1.0"              # Custom User-Agent string
connect_timeout_secs = 30              # Connection timeout for all protocols
tcp_keepalive_secs = 60                # TCP keepalive interval
proxy = "socks5://127.0.0.1:9050"    # Optional global SOCKS5 or HTTP proxy
max_redirects = 20                     # Maximum HTTP redirect depth
http_retry_count = 5                   # Number of retries for transient HTTP errors
http_retry_delay_secs = 2              # Delay between HTTP retries
dns_resolver = "system"              # DNS resolver: "system", "cloudflare", "google", or structured DoH/DoT
```

### BitTorrent
```toml
[bittorrent]
enabled = true                         # Global BitTorrent protocol toggle
max_peers_per_torrent = 50             # Max connections per torrent
max_overall_peers = 200                # Total global peer limit
request_pipeline_size = 10             # Number of in-flight requests per peer (pipelining)
dht_enabled = true                     # Enable Distributed Hash Table (BEP 5)
pex_enabled = true                     # Enable Peer Exchange (BEP 11)
lpd_enabled = false                    # Enable Local Peer Discovery (Multicast)
seed_ratio = 1.0                       # Target seed ratio (1.0 = upload as much as downloaded)
seed_time_mins = 0                     # Seed time limit in minutes (0 for infinite)
endgame_mode_enabled = true            # Parallel fetch final blocks from all peers (ADR 0035)
min_split_size_mb = 20                 # Minimum piece/segment size for splitting
max_connections_per_torrent = 100      # Hard cap on connections for a single swarm
```

### Advanced Storage Engine
```toml
[storage]
download_dir = "."                     # Default directory for finished files
cache_size_mb = 16                     # In-memory write-back cache size (reduces disk seek)
preallocate = true                     # Reserve disk space before starting (prevents fragmentation)
allocation_mode = "falloc"             # "none", "prealloc" (zeros), "falloc" (sparse/fast)
save_session_interval_secs = 10        # Interval to sync .aura control files to disk
read_ahead_kb = 128                    # Size of read-ahead buffer for seeding
write_buffer_kb = 256                  # Size of sequential write aggregation buffer
```

### Native VPN Integration
```toml
[vpn]
type_name = "wireguard"                # "openvpn" or "wireguard"
profile_path = "/etc/wireguard/wg0.conf"
auto_connect = false                   # Attempt to trigger VPN connection on startup
check_interval_secs = 5                # Frequency of tunnel health checks
force_tunnel = true                    # Refuse any traffic if VPN tunnel is not confirmed (Kill-switch)
```

### General & TUI Theme
```toml
[general]
log_level = "info"                     # "trace", "debug", "info", "warn", "error" (JSON-formatted in daemon)
log_path = "aura.log"                  # Optional path to log file (default is stdout)
check_integrity = true                 # Perform piece hash verification (SHA-1/SHA-256)
event_poll_interval_ms = 500           # Internal event loop sleep time
daemon_mode = false                    # Start in background as a system service

[general.theme]
primary = "#0000FF"                    # Primary color (Borders, Headers)
accent = "#00FFFF"                     # Accent color (Progress bars, Speed)
highlight = "#FFFF00"                  # Highlight color (Active selection)
background = "#000000"                 # Terminal background color
foreground = "#FFFFFF"                 # Main text color
success = "#00FF00"                    # Success status color
error = "#FF0000"                      # Error status color
warning = "#FFFF00"                    # Warning status color
```

## Hot-Reloading

Aura supports hot-reloading. Most changes to `Aura.toml` will be picked up by the daemon or CLI immediately without a restart.
