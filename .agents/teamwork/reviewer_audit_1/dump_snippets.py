import re

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

path_line_regex = re.compile(
    r"(`?)(/(?:home/jeff/aura/)?[a-zA-Z0-9_\-\./]+|deploy/[a-zA-Z0-9_\-\./]+|tools/[a-zA-Z0-9_\-\./]+|aura-[a-zA-Z0-9_\-\./]+)(?::([0-9,\s\-]+))?(`?)"
)

for row in table_lines:
    parts = [p.strip() for p in row.split("|")[1:-1]]
    req_id = re.search(r"\*\*(.+?)\*\*", parts[0]).group(1)
    name = parts[1]
    status_match = re.search(r"\*\*(.+?)\*\*", parts[2])
    status = status_match.group(1) if status_match else parts[2]
    code_cit = parts[3]
    test_cit = parts[4]

    print(f"\n=======================================================")
    print(f"REQUIREMENT: {req_id}: {name} [{status}]")
    print(f"Code Citation: {code_cit}")
    print(f"Test Citation: {test_cit}")

    matches = path_line_regex.findall(code_cit)
    for _, path_str, line_spec, _ in matches:
        path_str = path_str.strip("`").strip()
        if not path_str.startswith("/"):
            path_str = "/home/jeff/aura/" + path_str
        print(f"--- Snippet from {path_str} ({line_spec}) ---")
        try:
            with open(path_str, "r", errors="ignore") as f_in:
                file_lines = f_in.readlines()
            if line_spec:
                ranges = [r.strip() for r in line_spec.split(",") if r.strip()]
                for r in ranges:
                    if "-" in r:
                        s, e = r.split("-")
                        s_idx, e_idx = int(s) - 1, int(e)
                        snippet = "".join(file_lines[s_idx:min(e_idx, len(file_lines))])
                        print(snippet.strip()[:300] + ("..." if len(snippet.strip()) > 300 else ""))
                    else:
                        idx = int(r) - 1
                        if 0 <= idx < len(file_lines):
                            print(file_lines[idx].strip())
            else:
                print("".join(file_lines[:15]).strip() + "...")
        except Exception as e:
            print(f"Error reading file {path_str}: {e}")
