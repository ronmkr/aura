# ADR 0051: Docker Containerization

## Status
Implemented (2026-05-29, PR #144)

## Context
Aura is designed to be highly portable and capable of running as a headless daemon on servers, NAS devices, or personal workstations. While we provide pre-compiled binaries via GitHub Actions, setting up native runtimes (or dealing with dynamic linking issues related to `libdbus-1`) can still introduce friction for users who prefer containerized workloads. 

Providing an official Docker image solves distribution inconsistencies and allows users to seamlessly run the daemon (or the CLI) within isolated environments.

## Decision
We will provide an official `Dockerfile` to containerize the `aura` unified executable.

1. **Multi-Stage Build**: We will use a multi-stage Docker build.
    - **Builder Stage**: `rust:1.80-slim-bookworm` to compile the source code. This stage includes all heavy compilation toolchains and dependencies like `pkg-config` and `libdbus-1-dev`.
    - **Runtime Stage**: `debian:bookworm-slim` to host the final compiled binary. This ensures the final image size remains under 50MB while satisfying the dynamically linked `libdbus-1-3` requirement.
2. **Unified Entrypoint**: The `ENTRYPOINT` of the container will be set to `aura`.
    - By default, `docker run ronmkr/aura` will pass arguments to the `aura` executable.
    - Users can run the daemon via `docker run -p 6800:6800 ronmkr/aura daemon`.
    - Users can run the CLI downloader via `docker run -v $(pwd):/downloads ronmkr/aura "https://example.com/file"`.
3. **Security**: The runtime container will execute the process as a non-root user (`aurauser`) to follow standard container security practices.
4. **Volume Mounting**: A default `/downloads` volume is documented for users to map their host storage.

## Consequences

### Positive
- Zero-friction deployment for daemon environments (Docker, Kubernetes, Unraid, TrueNAS SCALE).
- Eliminates the need for users to manually install `libdbus-1-3` on minimal Linux distributions.
- Encourages decoupled service architectures where the daemon runs securely in the background.

### Negative
- Adds maintenance overhead for the `Dockerfile`.
- We must eventually set up a GitHub Action to automatically publish the Docker image to a registry (e.g., GHCR or Docker Hub) when a new release is tagged.

## Implementation
- **Docker Containerization**: Implemented via multi-stage `Dockerfile` in project root (2026-05-29, PR #144).
