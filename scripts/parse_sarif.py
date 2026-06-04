#!/usr/bin/env python3
import json
import glob
import os
import sys

def main():
    sarif_dir = os.environ.get("SARIF_DIR")
    if not sarif_dir:
        print("Error: SARIF_DIR environment variable is not set.", file=sys.stderr)
        sys.exit(1)
        
    if not os.path.isdir(sarif_dir):
        print(f"Error: SARIF_DIR '{sarif_dir}' is not a directory or does not exist.", file=sys.stderr)
        sys.exit(1)

    sarif_files = glob.glob(os.path.join(sarif_dir, "**/*.sarif"), recursive=True)
    findings = []
    
    for file_path in sarif_files:
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            
            for run in data.get("runs", []):
                rules_map = {}
                driver = run.get("tool", {}).get("driver", {})
                for rule in driver.get("rules", []):
                    rules_map[rule["id"]] = rule.get("shortDescription", {}).get("text", rule.get("name", ""))
                
                for result in run.get("results", []):
                    rule_id = result.get("ruleId")
                    message = result.get("message", {}).get("text", "")
                    level = result.get("level", "warning")
                    
                    locs = []
                    for loc in result.get("locations", []):
                        phys = loc.get("physicalLocation", {})
                        artifact = phys.get("artifactLocation", {})
                        uri = artifact.get("uri", "")
                        region = phys.get("region", {})
                        start_line = region.get("startLine", "")
                        if uri:
                            if start_line:
                                locs.append(f"{uri}:{start_line}")
                            else:
                                locs.append(uri)
                                
                    findings.append({
                        "rule_id": rule_id,
                        "description": rules_map.get(rule_id, message),
                        "level": level,
                        "location": ", ".join(locs) if locs else "Unknown"
                    })
        except Exception as e:
            print(f"Warning: Failed to parse {file_path}: {e}", file=sys.stderr)
            
    summary_md = ["## CodeQL Security Analysis Summary", ""]
    if not findings:
        summary_md.append("✅ **No security vulnerabilities or alerts were detected during the CodeQL scan.**")
    else:
        summary_md.append(f"⚠️ **Found {len(findings)} security alert(s) during scanning:**")
        summary_md.append("")
        summary_md.append("| Severity | Rule ID | Description | Location |")
        summary_md.append("| --- | --- | --- | --- |")
        for f in findings:
            emoji = "🔴" if f["level"] == "error" else "🟡"
            summary_md.append(f"| {emoji} {f['level'].upper()} | `{f['rule_id']}` | {f['description']} | {f['location']} |")
            
    summary_file = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_file:
        with open(summary_file, "a", encoding="utf-8") as sf:
            sf.write("\n".join(summary_md) + "\n")
        print("Successfully wrote CodeQL summary to GITHUB_STEP_SUMMARY.")
    else:
        print("Warning: GITHUB_STEP_SUMMARY environment variable not set. Summary output to stdout:")
        print("\n".join(summary_md))

if __name__ == "__main__":
    main()
