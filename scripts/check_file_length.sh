#!/bin/bash
# scripts/check_file_length.sh
# Enforces Aura's strict modularity standards:
# 1. No Rust source file in the workspace's src/ directories may exceed 350 lines.
# 2. Progressively enforces separate-test files: new/modified production source files must not contain inline test modules (mod tests).

LIMIT=350
FAILED=0

echo "========================================================="
echo "🔍 Starting Aura Modularity & 350-Line Limit Check..."
echo "========================================================="

# 1. Get the list of modified files in the current branch relative to origin/main (if in git environment)
CHANGED_FILES=()
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    # Default to origin/main as base branch, fallback to local main if not found
    BASE_BRANCH="origin/main"
    if ! git rev-parse "$BASE_BRANCH" >/dev/null 2>&1; then
        BASE_BRANCH="main"
    fi
    
    echo "ℹ️ Detecting changed files relative to $BASE_BRANCH..."
    while IFS= read -r f; do
        f_norm="${f#./}"
        if [[ "$f_norm" =~ \.rs$ && "$f_norm" == */src/* ]]; then
            CHANGED_FILES+=("$f_norm")
        fi
    done < <( { git diff --name-only "$BASE_BRANCH"...HEAD 2>/dev/null || git diff --name-only HEAD; git diff --name-only; git ls-files --others --exclude-standard; } 2>/dev/null )
fi

echo "ℹ️ Scanning all workspace Rust source files in src/ directories..."
echo "---------------------------------------------------------"

# 2. Iterate through all .rs files under src/
while IFS= read -r file; do
    [ -f "$file" ] || continue

    # Count lines in the file
    lines=$(wc -l < "$file" | tr -d '[:space:]')

    # Check 400-line limit
    if [ "$lines" -gt "$LIMIT" ]; then
        echo "❌ FAILURE: File '$file' exceeds the limit ($lines > $LIMIT lines)."
        FAILED=1
        continue
    fi

    # Check if this file is a new/modified production file and contains inline tests
    is_changed=0
    file_norm="${file#./}"
    for cf in "${CHANGED_FILES[@]}"; do
        cf_norm="${cf#./}"
        if [ "$cf_norm" = "$file_norm" ]; then
            is_changed=1
            break
        fi
    done

    # Exclude test files, mod.rs, and check for inline test module
    filename=$(basename "$file")
    if [[ "$filename" != *tests.rs && "$filename" != *test.rs && "$filename" != "mod.rs" ]]; then
        # Look for mod tests { or #[cfg(test)] mod tests {
        if grep -qE "mod tests\s*\{" "$file" 2>/dev/null; then
            if [ "$is_changed" -eq 1 ]; then
                echo "❌ FAILURE: Modified/New production file '$file' contains inline unit tests (mod tests)."
                echo "            Please extract unit tests into a separate dedicated test file (tests.rs)."
                FAILED=1
            else
                echo "⚠️ WARNING: Legacy production file '$file' contains inline unit tests. (Consider refactoring to tests.rs)"
            fi
            continue
        fi
    fi

    echo "✅ PASS: '$file' ($lines lines)"
done < <(find . -type f -name "*.rs" | grep "/src/")

echo "---------------------------------------------------------"
if [ "$FAILED" -ne 0 ]; then
    echo "❌ ERROR: Modularity check failed!"
    echo "   - Ensure no source files exceed 350 lines (refactor into submodules)."
    echo "   - Ensure all new/modified unit tests occupy their own separate files."
    echo "========================================================="
    exit 1
else
    echo "✅ SUCCESS: All modularity checks passed successfully!"
    echo "========================================================="
    exit 0
fi
