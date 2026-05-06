# ADR 0029: Resource Mapping and Task Chaining

## Status
Accepted

## Context
Advanced download scenarios (Torrents with many files, Metalinks) require flexible file placement and the ability to automatically transition from a metadata download to a data download.

## Decision
1. **Resource Mapper**: Each **Download Task** will have a mapper that decouples the logical file path (from the torrent) from the physical path on disk. This allows for features like "Select only File #3" and "Save File #1 as 'my_movie.mp4'".
2. **Conflict Handler**: The mapper will consult a `ConflictHandler` policy (Overwrite, Rename, Skip) before initializing the **Storage Engine** file handles.
3. **Chain Orchestrator**: We will implement a "Follow" mechanism where one task's completion event can trigger the creation of another task. This is essential for automatically starting a download once its `.torrent` or `.metalink` file has been fetched via HTTP.

## Alternatives Considered
- **Direct File Writing**: Writing files exactly as named in the torrent. *Rejected:* Prevents users from organizing their downloads or avoiding name collisions.
- **Manual Chaining**: Requiring the user to add the torrent file manually after it downloads. *Rejected:* Poor UX; `aria2` is famous for its "one-command" Magnet-to-File flow.

## Consequences
- **Pros**: Full parity with `aria2`'s complex multi-file and metadata workflows, plus improved file collision management.
- **Cons**: Adds another layer of indirection (The Mapper) between the **Protocol Worker** and the **Storage Engine**.
