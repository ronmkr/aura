# Getting Started

The primary way to interact with Aura is through the `aura`. This chapter provides a range of examples to help you get the most out of your download engine.

## Basic Downloads

To start a standard download, simply pass the URL:
```bash
aura "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"
```

Aura will automatically:
1. Resolve the filename from the URL.
2. Pre-allocate the full file size on disk.
3. Show a real-time progress bar with throughput and ETA.

## BitTorrent

Aura handles BitTorrent seamlessly. You can use Magnet links or local `.torrent` files:

```bash
# Using a Magnet Link
aura "magnet:?xt=urn:btih:..."

# Using a local torrent file
aura ./linux-distro.torrent
```

Aura will automatically connect to DHT and trackers to find peers and verify piece integrity using SHA-1 (v1) or SHA-256 (v2).

## Advanced Workflows

### Multi-Source Aggregation
If you have multiple mirrors for the same file, you can speed up the download by providing all of them:

```bash
aura "https://mirror1.org/f.zip" "https://mirror2.org/f.zip" "ftp://backup.com/f.zip"
```

Aura will split the file into ranges and fetch them simultaneously from all sources, using the **Racing Work Stealer** to bypass slow connections.

### Batch Downloads (Globbing)
Download multiple files using numeric ranges or sets:

```bash
# Download 10 parts of a split archive
aura "https://cdn.org/data/part[1-10].rar"

# Download the same file from multiple mirrors using set expansion
aura "https://mirror{us,eu,asia}.example.com/bigfile.iso"
```

### Metalink Support
For complex downloads with multiple mirrors and checksums, use a Metalink file:

```bash
aura "https://example.com/release.metalink"
```

## Controlling the Output

By default, Aura downloads to the current directory. Use the `--output` flag to rename the file:

```bash
aura "https://server.com/archive.tar.gz" --output backup.tgz
```

To change the download directory, use the `Aura.toml` configuration file. See the [Configuration](./configuration.md) chapter for details.
