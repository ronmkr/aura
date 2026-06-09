import json

try:
    with open('graphify-out/graph.json', 'r') as f:
        data = json.load(f)

    print("=== Potential Hardcoded Constants (Graphify) ===")
    count = 0
    for n in data.get('nodes', []):
        if n.get('file_type') == 'code':
            label = n.get('label', '')
            src = n.get('source_file', '')
            # Looking for CONSTANT_CASE variables or constants
            if label.isupper() and '_' in label and not label.endswith('()'):
                # Exclude config module, tests, and expected constant names
                if 'config' not in src and 'tests' not in src and 'test' not in src:
                    print(f" - {label} in {src}:{n.get('source_location', '?')}")
                    count += 1
    if count == 0:
        print("No uppercase constants found outside config.")
except Exception as e:
    print(f"Error: {e}")
