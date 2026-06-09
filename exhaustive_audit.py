import json

try:
    with open('graphify-out/graph.json', 'r') as f:
        data = json.load(f)

    print("=== Final Audit: Potentially Hardcoded Values ===")
    
    # 1. Look for remaining Uppercase constants
    print("\n[Uppercase Constants outside config/lib.rs/tests]")
    for n in data.get('nodes', []):
        if n.get('file_type') == 'code':
            label = n.get('label', '')
            src = n.get('source_file', '')
            if label.isupper() and '_' in label and not label.endswith('()'):
                if all(x not in src for x in ['config', 'tests', 'lib.rs', 'build.rs']):
                    print(f" - {label} in {src}")

    # 2. Look for hardcoded durations (often magic numbers)
    # This is trickier via AST alone if they aren't named constants,
    # so we'll look for labels that look like Duration::from_*
    print("\n[Duration calls found in graph]")
    for n in data.get('nodes', []):
        label = n.get('label', '')
        if 'Duration::from' in label:
            src = n.get('source_file', '')
            if all(x not in src for x in ['config', 'tests', 'test']):
                print(f" - {label} in {src}")

except Exception as e:
    print(f"Error: {e}")
