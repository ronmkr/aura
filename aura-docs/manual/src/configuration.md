# Configuration Reference: Deep Dive

Aura is highly tunable. This document provides an exhaustive, technical reference for every configuration key in `Aura.toml`.

## Table of Contents
1. [Configuration File Discovery](#configuration-file-location)
2. [[network] - Connectivity & Security](#network)
3. [[bandwidth] - Speed & Concurrency](#bandwidth)
4. [[bittorrent] - Protocol Tuning](#bittorrent)
5. [[storage] - I/O Optimization](#storage)
6. [[resource_mapping] - Path Logic](#resource_mapping)
7. [[hooks] - Automation](#hooks)
8. [[credentials] - Authentication](#credentials)
9. [[vpn] - Privacy Kill-switch](#vpn)
10. [[general] - Engine & UI](#general)

---

## Configuration File Location
Aura searches for `Aura.toml` in this order:
1.  **Direct Path**: Specified via the `--config` CLI flag.
2.  **Working Directory**: The folder where the `aura` command is executed.
3.  **User Config**: 
    - Linux/macOS: `~/.config/aura/Aura.toml`
    - Windows: `%AppData%\aura\Aura.toml`

---

## [network]
Manages how Aura interacts with the outside world.

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `interface` | String | `None` | Binds all outgoing traffic to a specific network interface. Use `eth0` for wired, `wlan0` for Wi-Fi, or `tun0` for VPNs. **Impact**: Prevents data leakage if multiple paths exist. |
| `local_addr` | String | `None` | Binds to a specific IP address on the local machine. Useful for servers with multiple IP aliases. |
| `listen_port` | u16 | `6881` | The port for incoming BitTorrent peer connections. Ensure this is forwarded in your router for "Green" DHT status. |
| `dht_port` | u16 | `6881` | The UDP port for Distributed Hash Table lookups. Often matched with `listen_port`. |
| `rpc_port` | u16 | `6800` | The port for the JSON-RPC 2.0 API. Used by the CLI and TUI to talk to the Daemon. |
| `rpc_secret` | String | `None` | Security token for the API. If set, clients must provide this via the `X-Aura-Token` HTTP header. |
| `user_agent` | String | `"Aura/0.1.0"` | The identifier sent to trackers and HTTP servers. Some restrictive trackers may require specific strings. |
| `connect_timeout_secs` | u64 | `30` | Seconds to wait for a TCP handshake. Lower this for faster failover on dead mirrors. |
| `tcp_keepalive_secs` | u64 | `60` | Interval for TCP keepalive packets to prevent silent connection drops by firewalls. |
| `proxy` | String | `None` | Global proxy. Supports `http://`, `https://`, and `socks5://`. **Performance**: SOCKS5 is preferred for BitTorrent. |
| `max_redirects` | usize | `20` | Maximum number of HTTP 3xx redirects to follow before failing a task. |
| `http_retry_count` | u32 | `5` | Retries for transient HTTP errors (500, 502, 503, 504). Uses exponential backoff. |
| `http_retry_delay_secs`| u64 | `2` | Initial delay for the first retry. Subsequent retries double this value. |
| `dns_resolver` | enum | `"system"` | See [DNS Configuration](#dns-configuration). Options: `"system"`, `"cloudflare"`, `"google"`, or specific IP. |

### DNS Configuration
Aura supports modern DNS protocols for privacy (ADR 0028).

**DNS-over-HTTPS (DoH):**
```toml
[network.dns_resolver]
type = "doh"
url = "https://cloudflare-dns.com/dns-query"
ips = ["1.1.1.1", "1.0.0.1"] # Bootstrap IPs to avoid chicken-and-egg resolver issues.
```

**DNS-over-TLS (DoT):**
```toml
[network.dns_resolver]
type = "dot"
server = "1.1.1.1"
tls_name = "cloudflare-dns.com"
port = 853
```

---

## [bandwidth]
Controls the flow of data to prevent network saturation.

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `global_download_limit`| u64 | `0` | Total bytes per second for all tasks combined. `0` = Unlimited. |
| `global_upload_limit` | u64 | `0` | Total bytes per second to upload. Crucial for avoiding ISP "bufferbloat". |
| `per_task_download_limit`| u64 | `0` | Hard cap for a single task, regardless of global capacity. |
| `per_task_upload_limit` | u64 | `0` | Hard cap for a single task's upload. |
| `max_concurrent_downloads`| usize | `10` | Tasks in `Downloading` phase. Others stay in `Waiting`. |
| `max_active_tasks` | usize | `5` | Total tasks allowed in the engine (Active + Seeding + Paused). |
| `min_connections_per_task`| usize | `16` | The lower bound for adaptive scaling. Aura won't drop below this even if speed is high. |
| `max_connections_per_task`| usize | `128` | The upper bound. Aura scales up to this if it detects a slow per-connection rate. |

---

## [bittorrent]
Low-level tuning for the BitTorrent protocol (BEP implementation).

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `enabled` | bool | `true` | Globally toggle BitTorrent support. |
| `max_peers_per_torrent`| usize | `200` | Limits CPU usage for massive swarms. |
| `max_overall_peers` | usize | `500` | Global peer limit to prevent OS file descriptor exhaustion. |
| `request_pipeline_size`| usize | `50` | Number of concurrent block requests sent to a single peer. **Impact**: Higher values are needed for high-latency (fiber) connections. |
| `dht_enabled` | bool | `true` | Enables Mainline DHT (BEP 5). Finds peers without a tracker. |
| `pex_enabled` | bool | `true` | Enables Peer Exchange (BEP 11). Learning about new peers from existing ones. |
| `lpd_enabled` | bool | `false` | Local Peer Discovery. Fast transfers with other Aura users on your LAN. |
| `seed_ratio` | f32 | `1.0` | Target upload ratio. `1.0` means "Share back what you took". |
| `seed_time_mins` | u32 | `0` | Time-based seeding limit. `0` = Seed forever until ratio is met. |
| `endgame_mode_enabled` | bool | `true` | When < 1% remains, Aura requests the same block from multiple peers. **Impact**: Prevents "Stuck at 99.9%" due to one slow peer. |
| `min_split_size_mb` | u64 | `20` | Only used for HTTP/FTP. Minimum size to split a file into parallel segments. |
| `max_connections_per_torrent`| usize| `200` | Hard cap for a single swarm's connection pool. |

---

## [storage]
Governs the asynchronous I/O engine.

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `download_dir` | String | `"."` | Absolute or relative path for finished files. |
| `cache_size_mb` | u32 | `16` | **Write-back Cache**. Aura buffers writes in RAM and flushes them sequentially. **Impact**: Greatly extends SSD/HDD lifespan for random-write protocols like BitTorrent. |
| `preallocate` | bool | `true` | If true, Aura reserves the full file size on disk before downloading byte 1. |
| `allocation_mode` | enum | `"falloc"` | **none**: No pre-allocation.<br>**prealloc**: Writes zeros to the whole file (Slow, stable).<br>**falloc**: Uses `posix_fallocate` (Instant on XFS/EXT4/NTFS). |
| `save_session_interval_secs`| u64 | `10` | Frequency for syncing `.aura` control files. Controls how much progress is lost on a power failure. |
| `read_ahead_kb` | u32 | `128` | Prefetches data into RAM when seeding. Reduces disk head movement. |
| `write_buffer_kb` | u32 | `256` | The chunk size for sequential flushes to the OS. |

---

## [resource_mapping]
Automated file management and renaming (ADR 0029).

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `default_conflict_policy`| enum | `"AutoRename"`| **AutoRename**: Appends `.1`, `.2` etc.<br>**Overwrite**: Replaces existing files.<br>**Skip**: Aborts the task. |

### Resource Mapping Rules
Rules are evaluated from top to bottom. The first match wins.

**Structure:**
```toml
[[resource_mapping.rules]]
condition = { type = "VARIANT", value = "CRITERIA" }
target = "TEMPLATE"
```

**Condition Variants:**
- `Extension`: Matches file extension (`"mp4"`, `"iso"`).
- `Domain`: Matches if the URL domain contains the string (`"google.com"`).
- `Protocol`: Matches `Http`, `Ftp`, or `BitTorrent`.
- `Regex`: Full regular expression match against the final filename.

**Target Template Placeholders:**
- `{name}`: Original filename.
- `{id}`: Numeric Task ID.
- `{ext}`: Extension.
- `{protocol}`: `https`, `ftp`, etc.
- `{host}`: `server.example.com`.
- `{domain}`: `example.com`.
- `{year}`, `{month}`, `{day}`: Current local date.

---

## [hooks]
Allows integration with external automation tools.

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `on_download_start` | String | `None` | Command run when the status changes to `Downloading`. |
| `on_download_complete`| String | `None` | Command run after 100% hash verification. |
| `on_download_error` | String | `None` | Command run when a task moves to the `Error` phase. |
| `on_download_pause` | String | `None` | Command run when a task is manually paused. |

**Variables**: Hooks can use environment variables like `$AURA_TASK_ID`, `$AURA_FILE_PATH`, and `$AURA_TASK_NAME`.

---

## [credentials]
Aura's unified vault for authentication.

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `netrc_path` | String | `None` | Path to a `.netrc` file. Aura uses this for HTTP Basic Auth and FTP logins. |
| `cookie_file` | String | `None` | Path to a Netscape-format `cookies.txt`. Crucial for downloading from forums or protected CDN nodes. |

---

## [vpn]
Native VPN integration for high-privacy environments (ADR 0038).

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `type_name` | String | `None` | `"wireguard"` or `"openvpn"`. |
| `profile_path` | String | `None` | Path to the config file (e.g., `/etc/wireguard/wg0.conf`). |
| `auto_connect` | bool | `false` | If true, Aura tries to up the interface on boot. |
| `check_interval_secs` | u64 | `5` | Frequency of health checks. |
| `force_tunnel` | bool | `false` | **The Kill-switch**. If the VPN interface drops, Aura pauses all tasks and closes all sockets within milliseconds. |

---

## [general]
Core engine behavior and aesthetic settings.

| Key | Type | Default | Detailed Description |
|-----|------|---------|----------------------|
| `log_level` | enum | `"info"` | `trace`, `debug`, `info`, `warn`, `error`. **Impact**: `trace` is extremely verbose and will slow down the engine. |
| `log_path` | String | `None` | Log file path. If `None`, logs go to `stderr`. |
| `check_integrity` | bool | `true` | If true, every single block is hash-verified. **Security**: Mandatory for BitTorrent. |
| `event_poll_interval_ms`| u64 | `500` | UI refresh rate. Lower = smoother progress bars, Higher = lower CPU usage. |
| `daemon_mode` | bool | `false` | If true, Aura detaches from the terminal and runs as a background service. |

### [general.theme] (TUI only)
Customizes the look of the Pilot Dashboard. Supports standard Hex codes (e.g., `"#FF00FF"`).
- `primary`: Borders and headers.
- `accent`: Progress bars and speed numbers.
- `highlight`: Selected row.
- `background`: Main background.
- `foreground`: Text color.
- `success`: Completed task bars.
- `error`: Failed task indicators.
- `warning`: Throttling/Warning alerts.
