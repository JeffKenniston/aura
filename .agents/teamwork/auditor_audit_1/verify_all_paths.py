import re
import os

with open('/home/jeff/aura/M0_M3_Audit_Report.md') as f:
    text = f.read()

tokens = re.findall(r'`([^`\n]+)`', text)

paths_found = set()
for t in tokens:
    t = t.strip()
    words = t.split()
    for w in words:
        if ('/' in w or any(w.endswith(ext) for ext in ['.rs', '.py', '.toml', '.json', '.md', '.sh', '.wit', '.ext4'])) and not w.startswith(('http://', 'https://', 'spiffe://', 'gemini-')):
            clean_w = re.sub(r':\d+(?:-\d+)?$', '', w).strip('()",;:\'[]')
            if clean_w and not clean_w.startswith(('-', '<', '>')):
                paths_found.add(clean_w)

missing = []
existing = []
for p in sorted(paths_found):
    full = p if p.startswith('/') else os.path.normpath(os.path.join('/home/jeff/aura', p))
    if os.path.exists(full):
        existing.append((p, full))
    else:
        missing.append((p, full))

print(f"Total paths checked: {len(paths_found)}")
print(f"Existing paths: {len(existing)}")
print(f"Missing paths count: {len(missing)}")
for m in missing:
    print(f"  MISSING: {m[0]} -> {m[1]}")
