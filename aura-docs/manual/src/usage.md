# Getting Started

The primary way to interact with Aura is through the `aura-cli`.

## Basic Downloads

Download a single file from HTTP:
```bash
aura-cli "https://example.com/file.iso"
```

Download from a BitTorrent Magnet link:
```bash
aura-cli "magnet:?xt=urn:btih:..."
```

## Multi-Source Aggregation

Aura can download the same file from multiple sources simultaneously:
```bash
aura-cli "https://mirror1.com/f.iso" "https://mirror2.com/f.iso"
```

Or use a Metalink file to automate this:
```bash
aura-cli "https://example.com/release.metalink"
```
