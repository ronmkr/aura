#!/usr/bin/env bash
#
# clean_cache.sh
# Safely removes Cargo build artifacts, application runtime data, OS junk, and temporary files.

set -e

# Change to the project root directory, resolving symlinks
PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$PROJECT_ROOT"

echo "Starting comprehensive cache cleanup in $PROJECT_ROOT..."

# 0. Terminate running processes to release file locks
echo -e "\nStopping running Aura processes..."
if pgrep -f "aura" > /dev/null; then
    pkill -f "aura" || true
    echo "Aura processes terminated."
else
    echo "   No Aura processes running."
fi

# 1. Cargo Build Cache
echo -e "\nCleaning Cargo build cache..."
if command -v cargo &> /dev/null; then
    cargo clean
    echo "Cargo clean complete."
else
    echo "Warning: Cargo is not installed or not in PATH. Skipping build cache cleanup."
fi

# 2. Application Data & Cache Directories
echo -e "\nRemoving application data, docs cache, and tooling directories..."
# Combine root directories into one array for cleaner logic
PROJECT_DIRS=(
    "downloads" 
    "download" 
    "out" 
    "db.sled" 
    ".antigravitycli" 
    "aura-docs/manual/book" 
    "aura-docs/manual/src/adr"
    "proptest-regressions"
)

for dir in "${PROJECT_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo "   Removed directory: $dir/"
    fi
done

# Find and remove recursive application cache directories
find . -type d \( -name ".aura" -o -name "*.sled" \) -prune -exec rm -rf {} +
echo "Application data directories removed."

# 3. Temporary Files, OS Junk, and Local Analysis
echo -e "\nRemoving logs, OS junk, editor temp files, and analysis artifacts..."

# Combine simple file deletions into a single fast find command
find . -type f \( \
    -name "*.log" -o \
    -name "*.part" -o \
    -name ".DS_Store" -o \
    -name ".DS_Store?" -o \
    -name "._*" -o \
    -name ".Spotlight-V100" -o \
    -name ".Trashes" -o \
    -name "Thumbs.db" -o \
    -name "ehthumbs.db" -o \
    -name "*.swp" -o \
    -name "*.swo" -o \
    -name "Cargo.lock.bak" -o \
    -name "metadata.json" -o \
    -name "unused_deps.txt" \
\) -delete

# Find and remove metadata/torrent files, explicitly excluding those in tests/ directories
find . -type f \( -name "*.aura" -o -name "*.torrent" -o -name "*.magnet" \) ! -path "*/tests/*" -delete

echo "Temporary files and junk removed."

echo -e "\nComprehensive cache cleanup complete!"