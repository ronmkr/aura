# Configuration Reference: Deep Dive

Aura is highly tunable. This document provides an exhaustive reference for every setting in `Aura.toml` using plain language.

## Table of Contents
1. [Configuration File Discovery](#configuration-file-location)
2. [[network] - Connectivity & Security](#network)
3. [[bandwidth] - Speed & Concurrency](#bandwidth)
4. [[bittorrent] - Protocol Tuning](#bittorrent)
5. [[storage] - I/O Optimization](#storage)
6. [[resource_mapping] - Path Logic](#resource_mapping)
7. [[bulk] - Batch Scanning](#bulk)
8. [[notifications] - OS Alerts](#notifications)
9. [[tui] - Dashboard Tuning](#tui)
10. [[hooks] - Automation](#hooks)
11. [[credentials] - Authentication](#credentials)
12. [[vpn] - Privacy Kill-switch](#vpn)
13. [[security] - Interface Hardening](#security)
14. [[monitoring] - Metrics & Health](#monitoring)
15. [[limits] - Architectural Boundaries](#limits)
16. [[general] - Engine & UI](#general)

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

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `interface` | Text | `None` | Binds all outgoing traffic to a specific network interface (e.g., `eth0`, `wlan0`, or `tun0`). |
| `local_addr` | IP Address | `None` | Binds to a specific IP address on your machine. |
| `bind_address` | IP Address | `"127.0.0.1"` | IP address to bind the RPC server (default: `127.0.0.1` for security). |
| `allowed_origins` | List of Texts | `["http://localhost", "http://127.0.0.1", "chrome-extension://"]` | Allowed CORS origins for API requests. |
| `listen_port` | Number | `6881` | The port for incoming BitTorrent peer connections. |
| `dht_port` | Number | `6881` | The UDP port for Distributed Hash Table lookups. |
| `rpc_port` | Number | `6800` | The port for the API (used by CLI and TUI). |
| `rpc_secret` | Text | `None` | A password for the API. If set, clients must provide it to connect. |
| `tls_cert` | File Path | `None` | Path to a security certificate for encrypted API connections. |
| `tls_key` | File Path | `None` | Path to a private key for encrypted API connections. |
| `user_agent` | Text | `"Aura/0.1.0"` | The name Aura identifies itself as to servers and trackers. |
| `connect_timeout_secs` | Time (Seconds) | `30` | How long to wait for a server to respond before giving up. |
| `tcp_keepalive_secs` | Time (Seconds) | `60` | How often to send "I'm still here" packets to keep connections alive. |
| `proxy` | Text | `None` | A proxy address (supports `http`, `https`, and `socks5`). |
| `max_redirects` | Number | `20` | Maximum number of times to follow a "moved" link. |
| `http_retry_count` | Number | `5` | How many times to retry a failed download. |
| `http_retry_delay_secs`| Time (Seconds) | `2` | Starting wait time between retries (doubles each time). |
| `happy_eyeballs_stagger_ms`| Time (ms) | `250` | Delay between trying different connection methods (IPv4 vs IPv6). |
| `http_buffer_capacity` | Number (Bytes) | `65536` | Memory reserved for each connection (64KB). |
| `http_concurrent_requests`| Number | `32` | Max number of simultaneous requests allowed globally. |
| `nat_refresh_interval_secs`| Time (Seconds) | `1800` | How often to refresh router port mappings (30 mins). |
| `tracker_timeout_secs` | Time (Seconds) | `10` | Timeout for standard tracker updates. |
| `udp_tracker_timeout_secs`| Time (Seconds) | `5` | Timeout for UDP tracker updates. |
| `roaming_reconnect_delay_ms`| Time (ms) | `500` | Wait time after switching internet (e.g. Wi-Fi to Ethernet). |
| `dns_resolver` | Option | `"system"` | How to look up website addresses (`system`, `cloudflare`, `google`). |

---

## [bandwidth]
Controls the flow of data to prevent slowing down your internet.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `global_download_limit`| Number (Bytes/s)| `0` | Total download speed for all tasks. `0` = Unlimited. |
| `global_upload_limit` | Number (Bytes/s)| `0` | Total upload speed for all tasks. `0` = Unlimited. |
| `per_task_download_limit`| Number (Bytes/s)| `0` | Speed cap for a single download. |
| `per_task_upload_limit` | Number (Bytes/s)| `0` | Upload cap for a single download. |
| `max_concurrent_downloads`| Number | `10` | How many files to download at once. |
| `max_active_tasks` | Number | `500` | Total limit of tasks (including paused and finished). |
| `min_connections_per_task`| Number | `16` | Minimum number of connections per file. |
| `max_connections_per_task`| Number | `128` | Maximum number of connections per file. |
| `adaptive_scaling_low_throughput`| Number (Bytes/s)| `102400` | Threshold to scale up connections. |
| `adaptive_scaling_high_throughput`| Number (Bytes/s)| `5242880` | Threshold to scale down connections. |

### [[bandwidth.schedule]]
Aura supports time-based bandwidth limits, allowing you to automatically throttle or unthrottle downloads at specific times (e.g., off-peak unlimited data).

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `from` | Text (HH:MM) | Mandatory | Start time for the schedule window (24h format). |
| `to` | Text (HH:MM) | Mandatory | End time for the schedule window (24h format). |
| `download_limit` | Number (Bytes/s)| `0` | Download speed cap during this window. |
| `upload_limit` | Number (Bytes/s)| `0` | Upload speed cap during this window. |
| `days` | List of Texts | `None` | Optional: Days the schedule applies (e.g., `["Mon", "Tue"]`). |
| `timezone` | Text | `"local"` | Optional: Timezone for the schedule (e.g., `UTC`). |

---

## [bulk]
Settings for batch operations and folder ingestion.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `max_scan_depth` | Number | `10` | Recursion limit for folder scanning (`add-folder`). |

---

## [notifications]
Native OS desktop notification settings.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `enabled` | Yes / No | `true` | Turn desktop notifications on or off. |
| `notify_on_complete` | Yes / No | `true` | Send alert when a download finishes. |
| `notify_on_error` | Yes / No | `true` | Send alert on fatal task errors. |
| `app_name` | Text | `"Aura"` | The name displayed in the notification header. |

---

## [tui]
Interactive dashboard settings.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `tick_rate_ms` | Time (ms) | `500` | Refresh rate for UI animations and charts. |
| `rpc_url` | Text | `None` | Default daemon RPC endpoint for the TUI to connect to. |

---

## [security]
Security hardening for the RPC interface and daemon.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `rpc_max_requests_per_minute` | Number | `120` | Rate limit for JSON-RPC requests per connection. |
| `rpc_max_connections` | Number | `32` | Maximum number of simultaneous RPC connections. |
| `ssrf_mitigation_enabled` | Yes / No | `true` | Prevent downloads from internal/private IP addresses. |

---

## [monitoring]
Metrics and health monitoring settings.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `metrics_enabled` | Yes / No | `false` | Enable the Prometheus metrics exporter. |
| `metrics_port` | Number | `9100` | Port for the `/metrics` endpoint. |
| `scrape_token` | Text | `None` | Bearer token required to scrape the `/metrics` endpoint. |

---

## [bittorrent]
Settings for fine-tuning BitTorrent downloads.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `enabled` | Yes / No | `true` | Turn BitTorrent support on or off. |
| `max_peers_per_torrent`| Number | `200` | Limit the number of people you connect to for one file. |
| `max_overall_peers` | Number | `500` | Total limit of connections across all torrents. |
| `request_pipeline_size`| Number | `50` | Number of data pieces to request at once from a peer. |
| `dht_enabled` | Yes / No | `true` | Find peers without a central server (DHT). |
| `pex_enabled` | Yes / No | `true` | Ask peers for other peers they know (PEX). |
| `lpd_enabled` | Yes / No | `false` | Find peers on your local home network (LPD). |
| `dht_save_interval_secs`| Time (Seconds) | `300` | How often to save the peer list to disk (5 mins). |
| `dht_ping_interval_secs`| Time (Seconds) | `600` | How often to check if peers are still online. |
| `dht_token_rotation_secs`| Time (Seconds) | `600` | Security token update frequency. |
| `dht_query_interval_secs`| Time (Seconds) | `120` | How often to search for new peers. |
| `dht_query_timeout_secs`| Time (Seconds) | `5` | How long to wait for a peer lookup response. |
| `tracker_polling_interval_secs`| Time (Seconds) | `60` | How often to ask the tracker for a new peer list. |
| `lpd_announce_interval_secs`| Time (Seconds) | `300` | How often to broadcast your presence to your home network. |
| `choker_interval_secs` | Time (Seconds) | `10` | How often to re-evaluate who to send data to. |
| `seeding.min_ratio` | Decimal Number | `1.0` | Target upload ratio to stop seeding. |
| `seeding.max_seeding_time`| Time (Seconds) | `3600` | Maximum time to seed in seconds. |
| `seeding.stop_on_either` | Yes / No | `true` | Stop seeding if either ratio or time limit is reached. |
| `endgame_mode_enabled` | Yes / No | `true` | Speeds up the final 1% of a download by asking everyone for the last pieces. |
| `min_split_size_mb` | Number (MB) | `20` | Smallest size allowed for a single download segment. |
| `max_connections_per_torrent`| Number| `200` | Hard connection limit for one torrent. |
| `streaming_metadata_pieces`| Number | `4` | Number of pieces at the beginning and end of a torrent to prioritize sequentially when streaming mode is enabled (for fast index/metadata loading). |

---

## [storage]
Controls how files are saved to your hard drive.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `download_dir` | Text | `"."` | Where finished files are moved. |
| `cache_size_mb` | Number (MB) | `16` | Amount of RAM used to "buffer" files before writing to disk. |
| `preallocate` | Yes / No | `true` | Reserve the full file space immediately to prevent "Disk Full" errors later. |
| `allocation_mode` | Option | `"falloc"` | Method for reserving disk space (`none`, `prealloc`, `falloc`). |
| `save_session_interval_secs`| Time (Seconds) | `10` | How often to save your progress so you can resume later. |
| `flush_interval_secs` | Time (Seconds) | `3` | Frequency of writing data from memory to the actual disk. |
| `io_deadline_ms` | Time (ms) | `500` | Target time for a single disk write to finish. |
| `read_ahead_kb` | Number (KB) | `128` | Pre-read data from disk into memory when uploading. |
| `write_buffer_kb` | Number (KB) | `256` | Size of individual data chunks written to disk. |

---

## [vpn]
Safety settings for using a VPN.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `type_name` | Text | `None` | The type of VPN you use (`wireguard` or `openvpn`). |
| `profile_path` | File Path | `None` | Path to your VPN configuration file. |
| `management_addr` | Text | `None` | VPN management address or interface control port. |
| `auto_connect` | Yes / No | `false` | Automatically try to connect to the VPN on startup. |
| `check_interval_secs` | Time (Seconds) | `5` | How often to check if your VPN is still connected. |
| `connect_timeout_secs` | Time (Seconds) | `5` | How long to wait for a VPN connection to start. |
| `force_tunnel` | Yes / No | `false` | **Kill-switch**: Pause everything if the VPN disconnects. |

---

## [hooks]
Automation and shell hooks on task lifecycle events.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `enabled` | Yes / No | `false` | Enable or disable execution of external shell hooks. |
| `on_download_complete` | Text | `""` | Command/script to execute when a download completes (receives Task ID). |

---

## [credentials]
Authentication credentials for secure HTTP, FTP, and Cloud Storage protocols (ADR 0013).

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `netrc_path` | File Path | `"~/.netrc"` | Path to the `.netrc` file containing machine logins and passwords. |
| `cookie_file` | File Path | `None` | Path to a Netscape-formatted cookie file for authenticated HTTP mirrors. |

### Cloud Storage Authentication

> [!NOTE]
> Cloud storage integration is compile-time gated: S3 requires compiling Aura with `--features s3`, and Google Drive/OneDrive require `--features gdrive`.

Aura supports secure credentials for S3, Google Drive, and OneDrive:

- **S3-Compatible Storage**: Handled via standard AWS SDK configuration. Ensure environment variables like `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_REGION` are set, or that a valid credentials file exists at `~/.aws/credentials`.
- **Google Drive**: Resolved from the `.netrc` file under hostnames `drive.google.com` or `googleapis.com`.
  - **API Key**: Set `login` to `"apikey"` and `password` to your API key.
  - **OAuth Bearer Token**: Set `password` to your OAuth2 Access Token.
- **OneDrive / SharePoint**: Resolved from the `.netrc` file under hostnames `graph.microsoft.com` or `onedrive.com`.
  - **OAuth Bearer Token**: Set `password` to your Microsoft Graph OAuth2 Access Token.
- **Usenet / NNTP**: Resolved from the `.netrc` file under the news server's hostname (e.g., `news.giganews.com`). Set `login` to your Usenet username and `password` to your news server password.

---

## [limits]
Defines administrative, network, and architectural constraints.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `allow_duplicate_uris` | Yes / No | `false` | Reject adding a task if its URI is already active. |
| `max_active_tasks` | Number | `100` | Maximum number of active/stored tasks inside the engine. |
| `event_channel_capacity` | Number | `1024` | Internal message channel capacity. |
| `command_channel_capacity` | Number | `128` | Control command queue capacity. |
| `storage_channel_capacity` | Number | `100` | Disk queue capacity. |
| `history_record_limit` | Number | `100000` | Hard cap on the number of historical records to keep. |
| `history_rotation_mb` | Number | `10.0` | Max size of history file before rotation. |
| `history_rotation_records`| Number | `10000` | Max records in history file before rotation. |
| `history_retention_records`| Number | `5000` | Number of records to keep after rotation. |
| `graceful_shutdown_timeout_secs` | Time (Seconds) | `5` | Maximum wait time for tasks on exit. |
| `pex_interval_secs` | Time (Seconds) | `60` | PEX peer request interval. |
| `bandwidth_scheduling_interval_secs` | Time (Seconds) | `60` | Scheduling check rate for limits. |
| `network_roaming_check_interval_secs` | Time (Seconds) | `5` | Interface poll rate for VPN swap. |
| `default_task_priority` | Number (0-5) | `3` | Default priority class for new tasks. |

---

## [general]
General app settings and visual theme.

| Setting | Value Type | Default | What it does |
|:---|:---|:---|:---|
| `log_level` | Option | `"info"` | How much detail to record in logs (`trace`, `debug`, `info`, `warn`, `error`). |
| `log_path` | Text | `None` | Where to save the log file. |
| `check_integrity` | Yes / No | `true` | Double-check every piece of data for errors. |
| `event_poll_interval_ms`| Time (ms) | `500` | How often to update the user interface (TUI). |
| `graceful_shutdown_timeout_secs`| Time (Seconds) | `5` | How long to wait for tasks to stop cleanly when closing. |
| `daemon_mode` | Yes / No | `false` | Run in the background without a window. |
