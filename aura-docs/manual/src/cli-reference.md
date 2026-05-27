# CLI Reference

The `aura` is the primary interface for most users. It is designed to be familiar to `aria2c` users while providing more powerful defaults.

## Usage

```bash
aura [OPTIONS] <URIS>...
```

### Arguments

- `<URIS>...`: One or more URIs to download. These can be HTTP(S) links, FTP links, magnet links, or paths to local `.torrent` or `.metalink` files.

### Options

- `-o, --output <FILE>`: (Optional) Override the output filename. By default, Aura infers the filename from the URI or metadata.
- `-h, --help`: Print help information.
- `-V, --version`: Print version information.

## URL Globbing

Aura supports powerful URL expansion (globbing), allowing you to download large batches of files with a single command.

### Numeric Ranges
Download a sequence of files:
```bash
aura "https://example.com/part[1-10].zip"
```
This expands to `part1.zip`, `part2.zip`, ..., `part10.zip`.

### Set Expansion
Download from a list of items:
```bash
aura "https://mirror{1,2,3}.com/linux.iso"
```

### Step Values
Download every second file:
```bash
aura "https://archive.org/data[0-100:10].bin"
```
This expands to `data0.bin`, `data10.bin`, ..., `data100.bin`.

## Multi-Source Downloads

If you provide multiple URIs for the same file, Aura will aggregate them into a single task and use the **Racing Work Stealer** to ensure maximum speed.

```bash
# Aggregating a fast mirror and a slow mirror
aura "https://fast-mirror.org/file.bin" "https://slow-mirror.org/file.bin"
```
