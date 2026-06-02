#!/bin/bash
# scripts/setup_hooks.sh
# Sets up local Git hooks for Aura developers.

set -e

HOOK_PATH=".git/hooks/pre-commit"

echo "Setting up pre-commit hook..."

cat << 'EOF' > "$HOOK_PATH"
#!/bin/bash
# Pre-commit hook to verify code quality before committing.
set -e

echo "=== Running Aura Pre-Commit Checks ==="

# Check formatting
echo "Checking formatting..."
cargo fmt --all -- --check

# Check lints
echo "Checking lints..."
cargo clippy --workspace -- -D warnings

# Check modularity rules (file lengths, inline test blocks, etc.)
echo "Checking codebase modularity..."
bash scripts/check_file_length.sh

echo "=== All Pre-Commit Checks Passed ==="
EOF

chmod +x "$HOOK_PATH"
echo "Pre-commit hook successfully installed at $HOOK_PATH"
