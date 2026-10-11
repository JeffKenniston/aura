import re
import os

with open("/home/jeff/aura/M0_M3_Audit_Report.md", "r") as f:
    text = f.read()

matrix_match = re.search(r"## 2\. Comprehensive Status Matrix.*?(?=## 3\. Deep Dive)", text, re.DOTALL)
matrix_text = matrix_match.group(0)

report_rows = re.findall(r"\|\s*\*\*([A-Z0-9-]+)\*\*\s*\|([^|]+)\|([^|]+)\|([^|]+)\|([^|]+)\|([^|]+)\|", matrix_text)

req_results = []
failures = []

for row in report_rows:
    req_id = row[0].strip()
    req_name = row[1].strip()
    status = row[2].replace("**", "").strip()
    primary_cites = row[3].strip()
    test_cites = row[4].strip()
    next_steps = row[5].strip()

    if status not in ("Complete", "Partial", "Missing"):
        failures.append((req_id, f"Invalid status: {status}"))
        continue

    if status in ("Complete", "Partial"):
        parts = re.split(r"<br>|\n", primary_cites)
        valid_cites_for_req = []
        for p in parts:
            p = p.strip()
            if not p:
                continue
            # match path:lines
            m = re.search(r"(`?(/home/jeff/aura/[^:`\s]+|[a-zA-Z0-9_\-\./]+(?:\.[a-zA-Z0-9]+))`?)(?::(\d+)(?:-(\d+))?)?", p)
            if not m:
                continue
            raw_path = m.group(2)
            sline = m.group(3)
            eline = m.group(4)

            full_path = raw_path if os.path.isabs(raw_path) else os.path.join("/home/jeff/aura", raw_path)
            if not os.path.exists(full_path):
                failures.append((req_id, f"Primary citation file missing: {raw_path} -> {full_path}"))
                continue

            with open(full_path, "r", encoding="utf-8", errors="ignore") as cf:
                file_lines = cf.readlines()
            n_lines = len(file_lines)

            # verify lines
            if sline:
                sl = int(sline)
                el = int(eline) if eline else sl
                # Allow line within file or within file + 1 (for trailing newline edge cases)
                if sl < 1 or sl > n_lines:
                    failures.append((req_id, f"Start line out of bounds: {sl} > {n_lines} for {raw_path}"))
                    continue
                # snippet
                end_idx = min(el, n_lines)
                snippet = "".join(file_lines[sl-1:end_idx]).strip()
                if not snippet:
                    failures.append((req_id, f"Empty snippet at lines {sl}-{el} for {raw_path}"))
                    continue
                valid_cites_for_req.append((raw_path, sl, el, len(snippet)))
            else:
                valid_cites_for_req.append((raw_path, None, None, n_lines))

        if not valid_cites_for_req:
            failures.append((req_id, f"No valid code citations found in primary_cites: {primary_cites}"))
        else:
            req_results.append((req_id, status, len(valid_cites_for_req)))

print(f"Total requirements evaluated: {len(report_rows)}")
print(f"Requirements with verified valid code citations: {len(req_results)}")
print(f"Failures: {len(failures)}")
if failures:
    for f in failures:
        print("  FAIL:", f)
