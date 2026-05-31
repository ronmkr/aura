# Task Chaining & Metadata Path Mapping

Aura includes a robust mechanism for task chaining and metadata-based directory mapping. These features allow you to automate complex multi-stage workflows and organize download folders automatically.

## Task Chaining & Follow-on Actions

Task chaining allows you to define dependencies between tasks or trigger follow-on tasks automatically when a download completes.

### Dependencies (depends_on)

A task can be configured to depend on one or more parent tasks. The orchestrator will prevent the dependent task from starting until all of its parent tasks have successfully completed.
- **Cycle Detection**: The system validates dependency graphs and detects/rejects circular dependencies to prevent deadlocks.
- **Auto-Maturation**: Once all parent tasks are complete, the dependent task automatically transitions from the waiting phase to the active downloading phase.

### Follow-on Actions (follow_on)

You can attach a follow-on action to a task. Currently supported actions include:
- **AutoStartTorrent**: If the downloaded file is a `.torrent` file, Aura will automatically register it as a new BitTorrent task and start the download.
- **AutoStartMetalink**: If the downloaded file is a `.metalink` file, Aura will automatically parse and launch it.
- **Custom(URI)**: Automatically triggers a new HTTP/S download for the specified URI upon task completion.

This allows you to download a torrent file via HTTP and have Aura automatically start downloading the actual torrent content without any manual intervention.

## Metadata-based Path Mapping

Aura's path mapping engine allows you to define flexible rules to automatically route downloaded files to specific subdirectories based on their metadata (such as file extensions, domains, protocol type, or regex matches).

### Supported Conditions

Rules are evaluated sequentially, and the first matching rule determines the target path. Supported matching conditions are:
- **Extension**: Matches the file extension (e.g. `mp4`, `zip`).
- **Domain**: Matches if any download mirror's domain contains the specified string.
- **Protocol**: Matches the protocol type (`Http`, `Ftp`, `BitTorrent`).
- **Regex**: Performs a regular expression match against the final task/file name.

### Dynamic Path Templates

When a rule matches, the task is saved to a path determined by the template string. The template supports the following placeholders:
- **{name}**: The original task or file name.
- **{id}**: The unique Task ID.
- **{ext}**: The file extension.
- **{protocol}**: The URI scheme (e.g., `https`, `ftp`).
- **{host}**: The remote hostname.
- **{domain}**: The registered domain name.
- **{year}**: Current local year (YYYY).
- **{month}**: Current local month (MM).
- **{day}**: Current local day (DD).

#### Example Configuration
```toml
[[resource_mapping.rules]]
condition = { type = "Extension", value = "mp4" }
target = "videos/{name}"

[[resource_mapping.rules]]
condition = { type = "Regex", value = ".*[0-9]+.*" }
target = "episodes/{name}"

[[resource_mapping.rules]]
condition = { type = "Extension", value = "zip" }
target = "archives/{id}_{ext}/{name}"
```

### Security & Directory Traversal Protection

To guarantee system safety, the mapping engine strictly sanitizes the final mapped path:
- **Sandbox Isolation**: All relative path segments like `..` (parent directory) or absolute prefixes (like `/`) are neutralized.
- **Component Filtering**: Only normal directory and file names are preserved, making it impossible for a malicious template or task name to escape the authorized download directory boundary.
