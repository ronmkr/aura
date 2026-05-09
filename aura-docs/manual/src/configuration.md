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

### Privacy
```toml
[network]
vpn_kill_switch = true
authorized_interface = "tun0"
```

## Hot-Reloading

Aura supports hot-reloading. Most changes to `Aura.toml` will be picked up by the daemon or CLI immediately without a restart.
