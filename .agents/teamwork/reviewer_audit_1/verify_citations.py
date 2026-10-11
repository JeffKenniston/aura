import re
import os

with open("/home/jeff/aura/M0_M3_Audit_Report.md") as f:
    lines = f.readlines()

table_lines = []
in_m0_m1 = False
for line in lines:
    if "### Phase M0: Workspace Initialization & Core Primitives" in line:
        in_m0_m1 = True
    elif "### Phase M2: Execution Substrate / Docker-Free Sandboxing" in line:
        in_m0_m1 = False
    if in_m0_m1 and line.startswith("| **"):
        table_lines.append(line.strip())

print(f"Total table rows extracted for M0/M1: {len(table_lines)}")

# We will parse each citation:
# It might be in format: `/path/to/file:start-end` or `/path/to/file:start-end, start-end` or `/path/to/file`

# Regex to match paths with optional line numbers:
# e.g., `/home/jeff/aura/aura-core/src/bus/mod.rs:61-69, 83-89`
# or `/home/jeff/aura/Cargo.toml:1-33`
# or `aura-sdk/tests/test_bus_client.py:22-38`
# or `/home/jeff/aura/aura-core/src/bus/rate_limiter.rs:5-19`

path_line_regex = re.compile(
    r"(`?)(/(?:home/jeff/aura/)?[a-zA-Z0-9_\-\./]+|deploy/[a-zA-Z0-9_\-\./]+|tools/[a-zA-Z0-9_\-\./]+|aura-[a-zA-Z0-9_\-\./]+)(?::([0-9,\s\-]+))?(`?)"
)

citations_checked = 0
errors = []

for row in table_lines:
    parts = [p.strip() for p in row.split("|")[1:-1]]
    req_id = re.search(r"\*\*(.+?)\*\*", parts[0]).group(1)
    status_match = re.search(r"\*\*(.+?)\*\*", parts[2])
    status = status_match.group(1) if status_match else parts[2]
    code_cit = parts[3]
    test_cit = parts[4]

    print(f"\n[{req_id}] ({status})")
    print(f"  Code citation: {code_cit}")
    print(f"  Test citation: {test_cit}")

    # Find citations in code_cit and test_cit
    for col_name, text in [("Code", code_cit), ("Test", test_cit)]:
        # Extract matches
        # Skip pure test command notes like `cargo test -p aura-core` or entire crate notes
        if "cargo test" in text or "Entire" in text or "Verified by" in text or "virtualenv" in text:
            # Check if there are still file citations inside
            pass
        matches = path_line_regex.findall(text)
        for _, path_str, line_spec, _ in matches:
            # Normalize path
            # strip trailing chars like `
            path_str = path_str.strip("`").strip()
            if not path_str.startswith("/"):
                path_str = "/home/jeff/aura/" + path_str
            
            # Check file existence
            if not os.path.isfile(path_str):
                errors.append(f"[{req_id}] {col_name}: File not found: {path_str}")
                print(f"    ERROR: File not found: {path_str}")
                continue
            
            citations_checked += 1
            with open(path_str, "r", errors="ignore") as f_in:
                file_lines = f_in.readlines()
            total_lines = len(file_lines)

            # Check lines
            if line_spec:
                # Can be multiple ranges like 51-61, 98-115
                ranges = [r.strip() for r in line_spec.split(",") if r.strip()]
                for r in ranges:
                    if "-" in r:
                        start_s, end_s = r.split("-")
                        try:
                            start_l = int(start_s)
                            end_l = int(end_s)
                            if start_l < 1 or end_l > total_lines:
                                errors.append(f"[{req_id}] {col_name}: Line range {r} out of bounds for {path_str} (total lines: {total_lines})")
                                print(f"    WARNING/ERROR: Line range {r} out of bounds for {path_str} ({total_lines} lines)")
                            else:
                                print(f"    OK: {path_str}:{r} (total {total_lines})")
                        except ValueError:
                            pass
                    else:
                        try:
                            l_num = int(r)
                            if l_num < 1 or l_num > total_lines:
                                errors.append(f"[{req_id}] {col_name}: Line {l_num} out of bounds for {path_str} (total lines: {total_lines})")
                                print(f"    WARNING/ERROR: Line {l_num} out of bounds for {path_str} ({total_lines} lines)")
                            else:
                                print(f"    OK: {path_str}:{l_num} (total {total_lines})")
                        except ValueError:
                            pass
            else:
                print(f"    OK: {path_str} (whole file, total {total_lines})")

print(f"\n==========================================")
print(f"Total citations checked: {citations_checked}")
print(f"Errors found: {len(errors)}")
for err in errors:
    print("  ", err)
