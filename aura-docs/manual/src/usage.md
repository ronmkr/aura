# Getting Started

The primary way to interact with Aura is through the `aura` CLI. This chapter provides a range of examples to help you get the most out of your download engine.

## Basic Downloads

To start a standard download, simply pass the URL:
```bash
aura "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"
```

Aura's **ProtocolDetector** (ADR 0065) automatically identifies the source type (HTTP, FTP, BitTorrent, or Metalink) and starts the appropriate worker.

---

## BitTorrent & Metalink

Aura handles Magnet links, local `.torrent` files, and `.metalink` files seamlessly:

```bash
# Using a Magnet Link
aura "magnet:?xt=urn:btih:..."

# Using a local metadata file
aura ./linux-distro.torrent
```

### Selective Downloading (ADR 0065)
For tasks with multiple files, you can choose exactly what to download:

1.  **List Files**: Find the indices of the files you want.
    ```bash
    aura show-files <GID>
    ```
2.  **Select Indices**: Start the download for specific files only.
    ```bash
    aura select-files <GID> --indices 0,2,5
    ```
*You can also perform this interactively in the **Pilot Dashboard (TUI)** by pressing `f` on any task.*

---

## Cloud Storage (ADR 0013)

Aura supports direct, range-based downloads from S3-compatible APIs and personal cloud storage providers (Google Drive and OneDrive):

```bash
# Download a file from an S3 bucket
aura "s3://my-bucket/dataset.tar.gz"

# Download a file from Google Drive
aura "gdrive://file-id-here"

# Download a file from OneDrive
aura "onedrive://item-id-here"
```

Aura's **ProtocolDetector** automatically identifies these cloud URIs and initiates the appropriate worker to perform chunked, parallel range-based downloads.

---

## Bulk Ingestion (ADR 0065)

Aura makes it easy to add hundreds of downloads at once:

### Ingest from a Folder
Scan a directory for all metadata files and add them to the queue:
```bash
aura add-from-folder ~/Downloads/torrents/ --recursive
```

### Ingest from a File
Add a list of URIs from a text file (one per line):
```bash
aura add-from-file ./backlog.txt
```

---

## Download History (ADR 0062)

Aura maintains a persistent log of every download. To view your history:
```bash
aura history --limit 20 --filter completed
```
*Use `aura history --format json` for integration with other scripts.*

---

## Advanced Workflows

### Multi-Source Aggregation
Aggregate bandwidth from multiple mirrors for a single task:
```bash
aura "https://mirror1.org/f.zip" "https://mirror2.org/f.zip" "ftp://backup.com/f.zip"
```

### Batch Downloads (Globbing)
Download sequences or sets of files:
```bash
# Download 10 parts of a split archive
aura "https://cdn.org/data/part[1-10].rar"

# Download from multiple mirrors using set expansion
aura "https://mirror{us,eu,asia}.example.com/bigfile.iso"
```

## Controlling the Output

By default, Aura downloads to the current directory. Use the `--output` flag to rename the file:
```bash
aura "https://server.com/archive.tar.gz" --output backup.tgz
```

To change the default download directory, use the `Aura.toml` configuration file. See the [Configuration](./configuration.md) chapter for details.
