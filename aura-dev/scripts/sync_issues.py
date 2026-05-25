import os
import re
import subprocess

ISSUES_DIR = "aura-docs/project/issues"

# Allowed repo labels (from gh label list)
ALLOWED_LABELS = {
    "bug", "documentation", "duplicate", "enhancement", "good first issue",
    "help wanted", "invalid", "question", "wontfix", "dependencies",
    "github_actions", "rust", "module:core", "module:storage", "status:stub",
    "status:unverified"
}

def map_labels(local_labels):
    mapped = []
    for label in local_labels:
        label = label.lower().strip()
        # Direct mappings
        if "bug" in label:
            mapped.append("bug")
        elif "enhancement" in label:
            mapped.append("enhancement")
        elif "storage" in label:
            mapped.append("module:storage")
        elif "core" in label or "orchestrator" in label:
            mapped.append("module:core")
        elif "test" in label:
            mapped.append("status:unverified")
        
        # Check standard labels
        if label in ALLOWED_LABELS:
            mapped.append(label)
            
    # Remove duplicates
    return list(set(mapped))

def parse_md_file(filepath):
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()
    
    # Extract YAML-like frontmatter
    match = re.match(r"^---\s*\n(.*?)\n---\s*\n(.*)$", content, re.DOTALL)
    if not match:
        return None
    
    frontmatter_raw = match.group(1)
    body = match.group(2).strip()
    
    frontmatter = {}
    for line in frontmatter_raw.splitlines():
        line = line.strip()
        if not line or ":" not in line:
            continue
        parts = line.split(":", 1)
        key = parts[0].strip()
        val = parts[1].strip()
        
        # Parse strings
        if (val.startswith('"') and val.endswith('"')) or (val.startswith("'") and val.endswith("'")):
            val = val[1:-1]
        
        # Parse lists like [type:bug, priority:critical, area:storage]
        if val.startswith("[") and val.endswith("]"):
            list_val = [x.strip() for x in val[1:-1].split(",") if x.strip()]
            frontmatter[key] = list_val
        else:
            frontmatter[key] = val
            
    return frontmatter, body

def main():
    if not os.path.exists(ISSUES_DIR):
        print(f"Directory {ISSUES_DIR} does not exist.")
        return
    
    files = sorted(os.listdir(ISSUES_DIR))
    for filename in files:
        if not filename.endswith(".md"):
            continue
        
        filepath = os.path.join(ISSUES_DIR, filename)
        parsed = parse_md_file(filepath)
        if not parsed:
            continue
        
        frontmatter, body = parsed
        title = frontmatter.get("title")
        labels = frontmatter.get("labels", [])
        status = frontmatter.get("status")
        
        # Skip already resolved issues
        if status == "RESOLVED" or filename.find("128") != -1:
            print(f"Skipping resolved issue: {title}")
            continue
        
        # Map labels to allowed ones
        mapped_labels = map_labels(labels)
        
        print(f"Creating GitHub issue: {title}")
        
        # Build gh command
        cmd = ["gh", "issue", "create", "--title", title, "--body", body]
        for l in mapped_labels:
            cmd.extend(["--label", l])
        
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            print(f"Successfully created issue: {result.stdout.strip()}")
        except subprocess.CalledProcessError as e:
            print(f"Failed to create issue '{title}': {e.stderr.strip()}")

if __name__ == "__main__":
    main()
