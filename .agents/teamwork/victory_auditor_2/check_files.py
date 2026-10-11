import re
import os

with open("/home/jeff/aura/M0_M3_Audit_Report.md", "r") as f:
    text = f.read()

# Find all file paths
pattern = r'(?:/home/jeff/aura/|`)([a-zA-Z0-9_\-\./]+\.(?:rs|py|toml|json|sh|md|wit|lock|yml|yaml))(?::\d+(?:-\d+)?)?`?'
matches = re.findall(pattern, text)

distinct_files = set(matches)
missing_files = []
existing_files = []

for f in distinct_files:
    clean = f.strip('` :')
    full_path = os.path.join('/home/jeff/aura', clean)
    if os.path.exists(full_path):
        existing_files.append(clean)
    else:
        missing_files.append(clean)

print(f"Total distinct file references found: {len(distinct_files)}")
print(f"Existing files: {len(existing_files)}")
print(f"Missing files count: {len(missing_files)}")
if missing_files:
    print("Missing files:", missing_files)
