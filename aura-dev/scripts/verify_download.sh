#!/bin/bash
# Aura-dev: verify_download.sh
# Analyzes a URL to see if it's a binary file or a landing page.

URI=$1
OUTPUT=${2:-"verify_result.tmp"}

if [ -z "$URI" ]; then
    echo "Usage: $0 <URI> [output_file]"
    exit 1
fi

echo "--- 🛡️ ANALYZING HEADERS ---"
curl -IL "$URI"

echo "--- 🔍 SNIFFING CONTENT (First 512 bytes) ---"
curl -s -L "$URI" | head -c 512 | hexdump -C

echo "--- 🧪 MIME CHECK ---"
MIME=$(curl -s -I -L "$URI" | grep -i "content-type" | tail -n 1)
echo "Final MIME Type: $MIME"

if [[ "$MIME" == *"text/html"* ]]; then
    echo "❌ WARNING: This is a LANDING PAGE (HTML)."
else
    echo "✅ SUCCESS: This appears to be a BINARY stream."
fi
