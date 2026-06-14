# Stage 1: Build Environment
FROM rust:1.80-slim-bookworm AS builder

# Install system dependencies required for compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libdbus-1-dev \
    libxcb1-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/aura

# Copy the Cargo files and workspace members first for better caching
COPY Cargo.toml Cargo.lock ./
COPY aura-core/Cargo.toml aura-core/
COPY aura-cli/Cargo.toml aura-cli/
COPY aura-daemon/Cargo.toml aura-daemon/
COPY aura-tui/Cargo.toml aura-tui/
COPY aura/Cargo.toml aura/

# Create dummy source files to satisfy cargo build for dependency caching
RUN mkdir -p aura-core/src aura-cli/src aura-daemon/src aura-tui/src aura/src \
    && touch aura-core/src/lib.rs aura-cli/src/lib.rs aura-daemon/src/lib.rs aura-tui/src/lib.rs \
    && echo "fn main() {}" > aura/src/main.rs

# We need to copy assets for aura-daemon as rust-embed requires them at compile time
COPY aura-daemon/web/ aura-daemon/web/

# Pre-build dependencies
RUN cargo build --release -p aura

# Copy the rest of the source code
COPY . .

# Build the unified binary in release mode
# We touch the main.rs to ensure the actual app is built after the dummy build
RUN touch aura/src/main.rs && cargo build --release -p aura

# Stage 2: Runtime Environment
FROM debian:bookworm-slim

# Install runtime dependencies
# - ca-certificates for HTTPS
# - libdbus-1-3 for dbus
# - libxcb for TUI/clipboard
# - wireguard-tools and iproute2 for VPN support (Decision-0038)
# - curl for healthchecks (Decision-0051)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libdbus-1-3 \
    libxcb1 \
    libxcb-render0 \
    libxcb-shape0 \
    libxcb-xfixes0 \
    wireguard-tools \
    iproute2 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for security
RUN useradd -m -s /bin/bash aurauser
USER aurauser
WORKDIR /home/aurauser

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/aura/target/release/aura /usr/local/bin/aura

# Expose the default RPC port for the daemon
EXPOSE 6800

# Provide a volume for downloads
VOLUME ["/downloads"]

# Add healthcheck to monitor the daemon (Decision-0051)
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:6800/health || exit 1

# Set the default entrypoint to the unified binary
ENTRYPOINT ["aura"]

# Default command if none is provided: print help
CMD ["--help"]
