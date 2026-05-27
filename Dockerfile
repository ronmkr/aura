# Stage 1: Build Environment
FROM rust:1.80-slim-bookworm AS builder

# Install system dependencies required for compilation (libdbus)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*

# Set up the working directory
WORKDIR /usr/src/aura

# Copy the Cargo files and workspace members
COPY . .

# Build the unified binary in release mode
RUN cargo build --release -p aura

# Stage 2: Runtime Environment
FROM debian:bookworm-slim

# Install runtime dependencies (ca-certificates for HTTPS, libdbus-1-3 for dbus)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libdbus-1-3 \
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

# Set the default entrypoint to the unified binary
ENTRYPOINT ["aura"]

# Default command if none is provided: print help
CMD ["--help"]
