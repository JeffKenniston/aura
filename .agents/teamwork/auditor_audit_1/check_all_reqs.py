import re
import os

with open('/home/jeff/aura/requirements.md') as f:
    master_lines = f.readlines()

with open('/home/jeff/aura/M0_M3_Audit_Report.md') as f:
    report_text = f.read()

# Parse master requirements
# | **ID** | Name | Description | Source |
master_reqs = {}
for line in master_lines:
    m = re.match(r'\|\s*\*\*([A-Z0-9_-]+)\*\*\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|', line)
    if m:
        req_id = m.group(1).strip()
        name = m.group(2).strip()
        desc = m.group(3).strip()
        source = m.group(4).strip()
        master_reqs[req_id] = {'name': name, 'desc': desc, 'source': source}

# Parse report requirements
# | Req ID | Requirement Name | Status | Primary Code Citation | Test Citation / Verification | Next Steps |
report_rows = re.findall(r'\|\s*\*\*([A-Z0-9_-]+)\*\*\s*\|\s*([^|]+)\|\s*\*\*?([A-Za-z]+)\*\*?\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|', report_text)
report_reqs = {}
for r in report_rows:
    req_id = r[0].strip()
    report_reqs[req_id] = {
        'name': r[1].strip(),
        'status': r[2].strip(),
        'primary': r[3].strip(),
        'test': r[4].strip(),
        'next_steps': r[5].strip()
    }

print(f"Master count: {len(master_reqs)}")
print(f"Report count: {len(report_reqs)}")

# Compare IDs
assert set(master_reqs.keys()) == set(report_reqs.keys()), "ID sets differ!"

# Inspect each citation in report
for req_id in master_reqs:
    m_info = master_reqs[req_id]
    r_info = report_reqs[req_id]
    
    # Check if name is consistent
    m_name_words = set(re.findall(r'\w+', m_info['name'].lower()))
    r_name_words = set(re.findall(r'\w+', r_info['name'].lower()))
    common_words = m_name_words.intersection(r_name_words)
    if not common_words:
        print(f"[{req_id}] Warning: name might not match: '{m_info['name']}' vs '{r_info['name']}'")

print("All requirement names checked.")
