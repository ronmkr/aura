# Real-World Applications: Resource Mapping & Task Chaining

This page demonstrates how to use Aura's Advanced Mapping Engine and Task Chaining to solve complex data management problems.

## 1. Automated Media Library Organization

### Scenario
You want all `.mp4` and `.mkv` files to be automatically sorted into a `videos/` folder, further categorized by the domain they were downloaded from and the current year.

### Configuration
```toml
[[resource_mapping.rules]]
condition = { Extension = "mp4" }
target = "videos/{domain}/{year}/{name}"

[[resource_mapping.rules]]
condition = { Extension = "mkv" }
target = "videos/{domain}/{year}/{name}"
```

### Result
- A download from `https://archive.org/movies/classic.mp4` will be saved to:
  `/downloads/videos/archive.org/2026/classic.mp4`
- A download from `https://my-nas.local/share/home_video.mkv` will be saved to:
  `/downloads/videos/my-nas.local/2026/home_video.mkv`

---

## 2. Multi-Step Data Pipeline (Task Chaining)

### Scenario
You need to download a daily manifest file (`manifest.json`) from a server. Once the manifest is downloaded, you want to automatically trigger the download of a large dataset described in that manifest.

### Implementation
Using the `Custom` follow-on action, you can link tasks together.

1. **Initial Task**: Download `manifest.json`.
2. **Follow-on**: Trigger download of `dataset.tar.gz`.

### Example (API usage)
```rust
let handle = engine.add_task(
    "https://api.data.gov/v1/daily/manifest.json",
    AddOptions {
        follow_on: Some(FollowOnAction::Custom("https://api.data.gov/v1/daily/dataset.tar.gz".to_string())),
        ..Default::default()
    }
).await?;
```

---

## 3. Protocol-Specific Sandboxing

### Scenario
For security or compliance reasons, you want all FTP downloads to be placed in a separate, isolated directory.

### Configuration
```toml
[[resource_mapping.rules]]
condition = { Protocol = "Ftp" }
target = "untrusted/ftp/{host}/{name}"
```

### Result
- `ftp://ftp.ubuntu.com/ls-lR.gz` goes to:
  `/downloads/untrusted/ftp/ftp.ubuntu.com/ls-lR.gz`

---

## 4. Versioned Archive Management

### Scenario
You are downloading nightly builds of a project and want to prevent them from overwriting each other by including the task ID and the date in the filename.

### Configuration
```toml
[[resource_mapping.rules]]
condition = { Regex = ".*nightly.*" }
target = "builds/{year}-{month}-{day}/{id}_{name}"
```

### Result
- `nightly-linux-x64.tar.gz` (ID: 4567) downloaded on 2026-05-31:
  `/downloads/builds/2026-05-31/4567_nightly-linux-x64.tar.gz`

---

## Supported Placeholders

| Placeholder | Description | Example |
|-------------|-------------|---------|
| `{name}` | Original filename | `movie.mp4` |
| `{id}` | Unique Task ID | `12345` |
| `{ext}` | File extension | `mp4` |
| `{protocol}`| URL Scheme | `https`, `ftp` |
| `{host}` | Remote hostname | `example.com` |
| `{domain}` | Registered domain | `example.com` |
| `{year}` | Current Year (local) | `2026` |
| `{month}` | Current Month (local) | `05` |
| `{day}` | Current Day (local) | `31` |
