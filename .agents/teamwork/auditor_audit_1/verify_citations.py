import os
import re

report_path = '/home/jeff/aura/M0_M3_Audit_Report.md'
with open(report_path) as f:
    text = f.read()

table_rows = re.findall(r'\|\s*\*\*([A-Z0-9_-]+)\*\*\s*\|\s*([^|]+)\|\s*\*\*?([A-Za-z]+)\*\*?\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|', text)
print(f"Parsed {len(table_rows)} table rows.")

# Path shouldn't include colon or whitespace or backtick or angle brackets or parentheses
file_pattern = re.compile(r'((?:/home/jeff/aura/|(?:\./)?(?:aura-core|aura-sdk|tools|deploy|docs|\.agents|Cargo\.toml)[^`\s<:()]+)(?:\.rs|\.py|\.toml|\.json|\.md|\.sh|\.wit|\.ext4)?)(?::(\d+)(?:-(\d+))?)?')

citations = []

for req_id, name, status, primary_cite, test_cite, next_steps in table_rows:
    for col_name, col_val in [('primary', primary_cite), ('test', test_cite)]:
        # Split by <br>, newline, or backtick boundaries
        parts = re.split(r'<br>|\n|`', col_val)
        for part in parts:
            part = part.strip()
            if not part:
                continue
            for match in file_pattern.finditer(part):
                filepath = match.group(1).strip()
                s_line = match.group(2)
                e_line = match.group(3)
                citations.append((req_id, col_name, match.group(0), filepath, s_line, e_line))

print(f"Total extracted citations: {len(citations)}")

missing_files = []
line_overflows = []
valid_citations = []

for req_id, col_name, raw, path, s_line, e_line in citations:
    cleaned_path = path.strip()
    if cleaned_path.startswith('/home/jeff/aura/'):
        full_path = cleaned_path
    else:
        full_path = os.path.normpath(os.path.join('/home/jeff/aura', cleaned_path))
    
    if not os.path.exists(full_path):
        missing_files.append((req_id, col_name, raw, full_path))
    else:
        if os.path.isdir(full_path):
            valid_citations.append((req_id, col_name, raw, full_path, 'DIR', None, None))
        else:
            with open(full_path, 'r', errors='ignore') as f:
                lines = f.readlines()
            total_lines = len(lines)
            if s_line:
                s = int(s_line)
                e = int(e_line) if e_line else s
                if s > total_lines or e > total_lines:
                    line_overflows.append((req_id, col_name, raw, full_path, total_lines, s, e))
                else:
                    valid_citations.append((req_id, col_name, raw, full_path, total_lines, s, e))
            else:
                valid_citations.append((req_id, col_name, raw, full_path, total_lines, None, None))

print(f"\n--- VERIFICATION RESULTS ---")
print(f"Valid citations count: {len(valid_citations)}")
print(f"Missing files count: {len(missing_files)}")
for mf in missing_files:
    print(f"  MISSING: {mf}")

print(f"Line overflows count: {len(line_overflows)}")
for lo in line_overflows:
    print(f"  LINE OVERFLOW: {lo}")
