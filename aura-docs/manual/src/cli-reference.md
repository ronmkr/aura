# CLI Reference

The `aura` CLI is a unified binary that provides both a standalone downloader and a management tool for the background daemon.

## General Usage

```bash
aura [OPTIONS] [URIS]...
aura <SUBCOMMAND> [ARGS]...
```

### Direct Download Mode
If you provide one or more URIs without a subcommand, Aura starts in **Standard CLI Mode**, downloading the files directly to the current directory with real-time progress bars.

```bash
aura "https://example.com/file.zip"
```

### Arguments

- `[URIS]...`: One or more URIs to download. 
    - **Supported Protocols**: `http`, `https`, `ftp`, `ftps`, `magnet`, `nntp`, `nntps`, `news`.
    - **Metadata Files**: Paths to local `.torrent`, `.metalink`, or `.meta4` files.
    - **Multi-Source**: If multiple URIs are provided, Aura automatically treats them as mirrors for a single task.

### Options

- `-o, --output <FILE>`: Override the output filename.
- `-p, --priority <0-5>`: Set task priority (0 is highest, default: 3).
- `-d, --depends-on <GIDS>`: List of Task GIDs (comma-separated) that must complete before this task starts.
- `--follow-on <URI>`: URI to automatically download after this task completes (Task Chaining).
- `--config <PATH>`: Use a custom `Aura.toml` configuration file.
- `--download-dir <PATH>`: Override the download directory for this session.
- `--limit <BYTES/S>`: Override the global download bandwidth limit.
- `--proxy <URL>`: Override the global proxy setting.
- `-v, -vv, -vvv`: Increase logging verbosity.
- `-h, --help`: Print help information.

---

## Subcommands

### `daemon`
Starts the Aura background daemon.

**Usage:** `aura daemon [OPTIONS]`
- `--bind-address <IP>`: IP to bind the RPC server (default: `127.0.0.1`).
- `--rpc-port <PORT>`: Port to bind the RPC server (default: `6800`).
- `--rpc-secret <TOKEN>`: Secret token for RPC authentication.
- `--tls-cert <PATH>`: Path to the TLS certificate file.
- `--tls-key <PATH>`: Path to the TLS private key file.
- `--generate-tls-cert`: Automatically generate self-signed TLS certificates.

### `tui`
Launches the **Pilot Dashboard**, the interactive terminal interface.

**Usage:** `aura tui`

### `status`
Displays real-time engine health, active bandwidth limits, and current schedules.

**Usage:** `aura status`

### `history`
View the log of completed and failed downloads (ADR 0062).

**Usage:** `aura history [OPTIONS]`
- `--limit <N>`: Number of records to show (default: 10).
- `--format <json|table>`: Output format.
- `--filter <completed|failed|removed>`: Filter by status.

### `add-from-folder`
Bulk ingest all metadata files (`.torrent`, `.metalink`) from a directory.

**Usage:** `aura add-from-folder <DIR> [OPTIONS]`
- `-r, --recursive`: Scan subdirectories recursively.

### `add-from-file`
Bulk ingest a list of URIs from a text file (one URI per line).

**Usage:** `aura add-from-file <PATH>`

### `show-files`
Display the file hierarchy within a BitTorrent or Metalink task.

**Usage:** `aura show-files <GID>`

### `select-files`
Select specific files to download within a multi-file task (ADR 0065).

**Usage:** `aura select-files <GID> --indices <ID1,ID2,...>`
- `-i, --indices`: Comma-separated list of file indices (get indices from `show-files`).

### `refresh`
Check for updates on a completed or active download using ETag or Last-Modified (Conditional GET).

**Usage:** `aura refresh <GID>`

### `recheck`
Force a full file integrity/hash validation scan of the target or `.part` files for a task (ADR 0068).

**Usage:** `aura recheck <GID>`

### `probe`
Run the **Allocation Prober** to identify the best disk allocation strategy for a filesystem.

**Usage:** `aura probe [DIR]`

### `service`
Manage the Aura daemon as a system service (systemd, launchd, or Windows Service) (ADR 0071).

**Usage:** `aura service <SUBCOMMAND>`
- `install [OPTIONS]`: Install the daemon as a system service.
    - `--config <PATH>`: Custom configuration file path to use for the service.
    - `--bind-address <IP>`: IP address to bind the RPC server.
    - `--rpc-port <PORT>`: Port to bind the RPC server.
    - `--user <USER>`: The username under which the service should run.
- `uninstall`: Uninstall the system service.
- `start`: Start the system service.
- `stop`: Stop the system service.
- `status`: Check the status of the system service.

### `feed`
Manage RSS/Atom feed subscriptions for automated download ingestion (ADR 0070).

**Usage:** `aura feed <SUBCOMMAND>`
- `add <URL> [OPTIONS]`: Subscribe to an RSS/Atom feed.
    - `--name <NAME>`: Custom name for this subscription.
    - `--poll-interval <MINUTES>`: Custom polling interval in minutes (default: 30).
    - `-f, --filter <PATTERN>`: Title matching filters (regular expressions or strings).
    - `-c, --category <CATEGORY>`: Category matching filters (can be specified multiple times).
    - `--max-size <BYTES>`: Maximum item size limit in bytes.
- `remove <NAME_OR_URL>`: Unsubscribe from a feed by URL or name.
- `list`: List all subscribed feeds.
- `refresh`: Force a refresh/poll of all feeds immediately.

---

## URL Globbing

Aura supports powerful URL expansion (globbing).

### Numeric Ranges
`aura "https://example.com/part[1-10].zip"`

### Numeric Padding
`aura "https://example.com/image[001-099].jpg"`

### Set Expansion
`aura "https://mirror{1,2,3}.com/linux.iso"`

### Step Values
`aura "https://archive.org/data[0-100:10].bin"` (Expands to 0, 10, 20...)

### Crawler Seed Expansion
Aura's recursive crawler (used during HTML page asset discovery) expands globbing patterns in the start URL, running crawls across all target variations:
`aura "https://example.com/gallery-[1-5].html"`

---

## Multi-Source Downloads (Mirror Aggregation)

Aura aggregates bandwidth from multiple sources automatically.

```bash
aura "https://mirror-a.org/ubuntu.iso" "ftp://mirror-b.net/ubuntu.iso"
```
- **Racing Work Stealer**: Faster mirrors "steal" chunks from slower ones.
- **Protocol Mixing**: Mix HTTP, FTP, BitTorrent, and Usenet (NNTP) sources seamlessly.
