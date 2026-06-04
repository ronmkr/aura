# CLI Reference

The `aura` CLI is the primary interface for most users. It is designed to be familiar to standard download manager users while providing modern defaults and advanced automation.

## General Usage

```bash
aura [OPTIONS] [URIS]...
aura <SUBCOMMAND>
```

### Direct Download Mode
If you provide one or more URIs without a subcommand, Aura starts in **Standard CLI Mode**, downloading the files directly to the current directory with real-time progress bars.

```bash
aura "https://example.com/file.zip"
```

### Arguments

- `[URIS]...`: One or more URIs to download. 
    - **Supported Protocols**: `http`, `https`, `ftp`, `ftps`, `magnet`.
    - **Metadata Files**: Paths to local `.torrent`, `.metalink`, or `.meta4` files.
    - **Multi-Source**: If multiple URIs are provided, Aura automatically treats them as mirrors for a single task and uses the **Racing Work Stealer** to maximize throughput.

### Options

- `-o, --output <FILE>`: (Optional) Override the output filename. By default, Aura infers the filename from the URI or metadata.
- `-h, --help`: Print help information.
- `-V, --version`: Print version information.

---

## Subcommands

### `daemon`
Starts the Aura background daemon. This is required for remote control via JSON-RPC or if you want to leave tasks running in the background.

**Usage:**
```bash
aura daemon [OPTIONS]
```

**Options:**
- `--rpc-port <PORT>`: Port to bind the JSON-RPC server (default: `6800`).
- `--rpc-secret <TOKEN>`: Secret token for RPC authentication.

### `tui`
Launches the **Pilot Dashboard**, a full-screen Terminal User Interface for managing all active and completed tasks across the engine.

**Usage:**
```bash
aura tui
```

### `history`
Displays the download history of completed, stopped, and failed tasks recorded in the history log.

**Usage:**
```bash
aura history [OPTIONS]
```

**Options:**
- `--limit <N>`: Limit the number of returned history records.
- `--format <FORMAT>`: Output format (`json` or `table`).
- `--filter <FILTER>`: Filter by phase status (`failed` or `completed`).

---

## URL Globbing

Aura supports powerful URL expansion (globbing), allowing you to download large batches of files with a single command. Globbing works for all protocols.

### Numeric Ranges
Download a sequence of files:
```bash
aura "https://example.com/part[1-10].zip"
```
*Expands to `part1.zip`, `part2.zip`, ..., `part10.zip`.*

### Numeric Padding
Maintain leading zeros in sequences:
```bash
aura "https://example.com/image[001-099].jpg"
```
*Expands to `image001.jpg`, `image002.jpg`, etc.*

### Set Expansion
Download from a list of items:
```bash
aura "https://mirror{1,2,3}.com/linux.iso"
```
*Expands to `mirror1.com`, `mirror2.com`, and `mirror3.com`.*

### Step Values
Download every N-th file:
```bash
aura "https://archive.org/data[0-100:10].bin"
```
*Expands to `data0.bin`, `data10.bin`, ..., `data100.bin`.*

---

## Multi-Source Downloads (Mirror Aggregation)

Aura excels at aggregating bandwidth from multiple sources. If you provide multiple URIs for the same file, Aura treats them as a single logical task.

```bash
# Aggregating a fast mirror and a slow mirror
aura "https://mirror-a.org/ubuntu.iso" "ftp://mirror-b.net/ubuntu.iso"
```

**Key Features:**
- **Racing Work Stealer**: If one source is lagging, other workers will "steal" its assigned chunks to finish the download faster.
- **Protocol Mixing**: Mix HTTP, FTP, and BitTorrent sources for the same file seamlessly.
- **Failover**: If one mirror goes down, the task continues uninterrupted with the remaining sources.

---

## Advanced Automation (Environment Variables)

Aura populates specific environment variables when executing [Lifecycle Hooks](configuration.md#hooks):

| Variable | Description |
|----------|-------------|
| `$AURA_TASK_ID` | The unique internal ID of the task. |
| `$AURA_TASK_NAME` | The logical name of the download. |
| `$AURA_FILE_PATH` | The absolute path to the downloaded file on disk. |
| `$AURA_TENANT_ID` | The ID of the tenant (if multi-tenancy is active). |
