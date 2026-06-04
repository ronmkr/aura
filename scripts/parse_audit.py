#!/usr/bin/env python3
import json
import os
import subprocess
import sys

def sanitize_description(desc):
    if not desc:
        return ""
    desc = desc.strip()
    
    # Remove any trailing markdown code block closers that are not opened
    # (Fixes upstream formatting bug in RUSTSEC-2025-0057)
    if desc.endswith("```"):
        # Check if there is an unclosed triple backtick
        if desc.count("```") % 2 != 0:
            desc = desc[:-3].strip()
            
    # General safety check to ensure balanced triple backticks
    if desc.count("```") % 2 != 0:
        desc = desc.replace("```", "::triple-backtick::") # Avoid breaking markdown blocks
        
    return desc

def format_entry(entry, entry_type_name, emoji):
    advisory = entry.get("advisory", {})
    package = entry.get("package", {})
    
    advisory_id = advisory.get("id", "Unknown ID")
    title = advisory.get("title", "No Title")
    description = sanitize_description(advisory.get("description", ""))
    url = advisory.get("url") or entry.get("url")
    
    md = []
    md.append(f"### {emoji} {advisory_id}: {title}")
    md.append("")
    md.append("| Details | |")
    md.append("| --- | --- |")
    md.append(f"| **Package** | `{package.get('name', 'unknown')}` |")
    md.append(f"| **Version** | `{package.get('version', 'unknown')}` |")
    md.append(f"| **Type** | {entry_type_name} |")
    if url:
        md.append(f"| **URL** | <{url}> |")
        
    patched = entry.get("versions", {}).get("patched", [])
    if patched:
        md.append(f"| **Patched** | {' OR '.join(patched)} |")
    else:
        md.append("| **Patched** | n/a |")
        
    md.append("")
    md.append("> " + description.replace("\n", "\n> "))
    md.append("")
    return "\n".join(md)

def main():
    # Run cargo audit --json
    try:
        result = subprocess.run(["cargo", "audit", "--json"], capture_output=True, text=True, check=False)
    except Exception as e:
        print(f"Error executing cargo audit: {e}", file=sys.stderr)
        sys.exit(1)
        
    exit_code = result.returncode
    
    try:
        data = json.loads(result.stdout)
    except Exception as e:
        print("Error parsing cargo audit JSON output. Raw stdout:", file=sys.stderr)
        print(result.stdout, file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        sys.exit(exit_code or 1)
        
    summary_md = ["# Rustsec Dependency Audit Report", ""]
    
    vulnerabilities = data.get("vulnerabilities", {})
    vuln_list = vulnerabilities.get("list", [])
    vuln_count = vulnerabilities.get("count", 0)
    
    warnings_dict = data.get("warnings", {})
    # Flatten all warnings lists into one list
    warning_list = []
    for warning_type, entries in warnings_dict.items():
        for entry in entries:
            entry["warning_type"] = warning_type
            warning_list.append(entry)
            
    if vuln_count == 0 and not warning_list:
        summary_md.append("✅ **No vulnerabilities or warnings found in dependencies.**")
    else:
        summary_md.append(f"Found {vuln_count} vulnerability/vulnerabilities and {len(warning_list)} warning(s).")
        summary_md.append("")
        
        if vuln_list:
            summary_md.append("## 🔴 Vulnerabilities Detected")
            summary_md.append("")
            for vuln in vuln_list:
                summary_md.append(format_entry(vuln, "Vulnerability", "🔴"))
                
        if warning_list:
            summary_md.append("## ⚠️ Informational Warnings")
            summary_md.append("")
            for warning in warning_list:
                kind = warning.get("warning_type", "warning").capitalize()
                summary_md.append(format_entry(warning, kind, "⚠️"))
                
    summary_file = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_file:
        with open(summary_file, "w", encoding="utf-8") as sf:
            sf.write("\n".join(summary_md) + "\n")
        print("Successfully wrote Rustsec audit summary to GITHUB_STEP_SUMMARY.")
    else:
        print("Warning: GITHUB_STEP_SUMMARY not set. Output:")
        print("\n".join(summary_md))
        
    sys.exit(exit_code)

if __name__ == "__main__":
    main()
