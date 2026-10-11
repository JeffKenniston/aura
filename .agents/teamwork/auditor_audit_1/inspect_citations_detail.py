import re
import os

with open('/home/jeff/aura/M0_M3_Audit_Report.md') as f:
    text = f.read()

# Parse all rows
table_rows = re.findall(r'\|\s*\*\*([A-Z0-9_-]+)\*\*\s*\|\s*([^|]+)\|\s*\*\*?([A-Za-z]+)\*\*?\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|', text)

file_pattern = re.compile(r'((?:/home/jeff/aura/|(?:\./)?(?:aura-core|aura-sdk|tools|deploy|docs|\.agents|Cargo\.toml)[^`\s<:()]+)(?:\.rs|\.py|\.toml|\.json|\.md|\.sh|\.wit|\.ext4)?)(?::(\d+)(?:-(\d+))?)?')

def inspect_citation(raw_path, s_line, e_line):
    cleaned = raw_path.strip()
    if cleaned.startswith('/home/jeff/aura/'):
        full = cleaned
    else:
        full = os.path.normpath(os.path.join('/home/jeff/aura', cleaned))
    
    if not os.path.exists(full):
        return False, "FILE_DOES_NOT_EXIST", full, ""
    
    if os.path.isdir(full):
        return True, "DIRECTORY", full, ""
        
    with open(full, 'r', errors='ignore') as f:
        lines = f.readlines()
    
    if s_line:
        s = int(s_line)
        e = int(e_line) if e_line else s
        # get slice
        snippet = "".join(lines[max(0, s-1):min(len(lines), e)])
        return True, f"LINES_{s}_{e}", full, snippet
    return True, "WHOLE_FILE", full, "".join(lines[:10])

results = []
for req_id, name, status, primary_cite, test_cite, next_steps in table_rows:
    req_res = {'id': req_id, 'name': name.strip(), 'status': status.strip(), 'primary': [], 'test': []}
    
    # parse primary
    for m in file_pattern.finditer(primary_cite):
        exists, info, path, snippet = inspect_citation(m.group(1), m.group(2), m.group(3))
        req_res['primary'].append((m.group(0), exists, info, path, snippet))
        
    # parse test
    for m in file_pattern.finditer(test_cite):
        exists, info, path, snippet = inspect_citation(m.group(1), m.group(2), m.group(3))
        req_res['test'].append((m.group(0), exists, info, path, snippet))
        
    results.append(req_res)

# Check for non-existent files in citations
print("=== CITATIONS INSPECTION ===")
for r in results:
    for cite_type in ['primary', 'test']:
        for raw, exists, info, path, snippet in r[cite_type]:
            if not exists:
                print(f"[{r['id']}] NON-EXISTENT FILE in {cite_type}: {raw} -> {path}")

