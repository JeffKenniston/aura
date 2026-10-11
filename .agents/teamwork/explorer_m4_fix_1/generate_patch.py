with open("aura-core/src/hypervisor/firecracker.rs", "r") as f:
    lines = f.readlines()

# 1. Quoted cwd
idx_script = None
for i, line in enumerate(lines):
    if "let script = format!(" in line and "cd {} 2>/dev/null || true" in lines[i+1]:
        idx_script = i
        break
assert idx_script is not None, "idx_script not found"

lines[idx_script+1] = lines[idx_script+1].replace("cd {} 2>/dev/null || true", 'cd \\"{}\\" 2>/dev/null || true')
lines[idx_script+2] = lines[idx_script+2].replace("cwd_clause,", "escaped_cwd,")
lines.insert(idx_script, '        let escaped_cwd = cwd_clause.replace(\'"\', "\\\\\\"");\n')

# 2. Environment replacement
# Find 'if let Some(post) = env_bytes {'
idx_env = None
for i, line in enumerate(lines):
    if "if let Some(post) = env_bytes {" in line:
        idx_env = i
        break
assert idx_env is not None, "idx_env not found"

# insert let mut new_env
lines.insert(idx_env + 1, "            let mut new_env = std::collections::HashMap::new();\n")
for j in range(idx_env + 2, idx_env + 20):
    if "self.current_env.insert(k.to_string(), v.to_string());" in lines[j]:
        lines[j] = "                        new_env.insert(k.to_string(), v.to_string());\n"
        break

# find the closing brace of `for entry in post_trimmed.split(|&b| b == 0) {`
# The for loop starts around idx_env + 3. It has 3 nested blocks: for, if, if.
# So we count braces:
depth = 0
for_idx = None
for j in range(idx_env + 1, idx_env + 25):
    if "for entry in post_trimmed.split" in lines[j]:
        for_idx = j
        break
assert for_idx is not None, "for loop not found"

brace_count = 0
for j in range(for_idx, for_idx + 20):
    brace_count += lines[j].count("{") - lines[j].count("}")
    if brace_count == 0:
        # j is the line with the closing brace of the for loop
        lines.insert(j + 1, "            self.current_env = new_env;\n")
        break

# 3. close(&mut self)
for i, line in enumerate(lines):
    if "pub async fn close(mut self)" in line:
        lines[i] = "    pub async fn close(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {\n"
        break

# 4. BashSessionManager struct
for i, line in enumerate(lines):
    if "sessions: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, BashSessionHandle>>>," in line:
        lines[i] = "    sessions: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<BashSessionHandle>>>>>,\n"
        break

# 5. BashSessionManager methods
mgr_start = None
mgr_end = None
for i, line in enumerate(lines):
    if "pub async fn execute_in_session(" in line:
        mgr_start = i
    if "pub async fn session_count(" in line:
        mgr_end = i
        break
assert mgr_start is not None and mgr_end is not None, "mgr start/end not found"

new_methods = [
    "    pub async fn execute_in_session(\n",
    "        &self,\n",
    "        session_id: &str,\n",
    "        command: &str,\n",
    "    ) -> Result<BashCommandOutput, Box<dyn std::error::Error + Send + Sync>> {\n",
    "        let session_arc = {\n",
    "            let mut sessions = self.sessions.lock().await;\n",
    "            if let Some(arc) = sessions.get(session_id) {\n",
    "                arc.clone()\n",
    "            } else {\n",
    "                let session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;\n",
    "                let arc = std::sync::Arc::new(tokio::sync::Mutex::new(session));\n",
    "                sessions.insert(session_id.to_string(), arc.clone());\n",
    "                arc\n",
    "            }\n",
    "        };\n",
    "\n",
    "        let mut session = session_arc.lock().await;\n",
    "        if session.last_accessed.elapsed() > self.inactivity_timeout {\n",
    "            let _ = session.close().await;\n",
    "            let mut new_session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;\n",
    "            let output = new_session.execute(command, self.inactivity_timeout).await?;\n",
    "            *session = new_session;\n",
    "            Ok(output)\n",
    "        } else {\n",
    "            session.execute(command, self.inactivity_timeout).await\n",
    "        }\n",
    "    }\n",
    "\n",
    "    /// Closes and tears down an active session by ID.\n",
    "    pub async fn close_session(&self, session_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {\n",
    "        let maybe_arc = {\n",
    "            let mut sessions = self.sessions.lock().await;\n",
    "            sessions.remove(session_id)\n",
    "        };\n",
    "        if let Some(arc) = maybe_arc {\n",
    "            let mut session = arc.lock().await;\n",
    "            session.close().await?;\n",
    "            Ok(true)\n",
    "        } else {\n",
    "            Ok(false)\n",
    "        }\n",
    "    }\n",
    "\n",
    "    /// Sweeps and terminates any sessions that have exceeded the inactivity timeout.\n",
    "    pub async fn cleanup_expired_sessions(&self) -> usize {\n",
    "        let mut expired = Vec::new();\n",
    "        {\n",
    "            let mut sessions = self.sessions.lock().await;\n",
    "            let timeout = self.inactivity_timeout;\n",
    "            let mut to_remove = Vec::new();\n",
    "            for (sid, arc) in sessions.iter() {\n",
    "                if let Ok(guard) = arc.try_lock() {\n",
    "                    if guard.last_accessed.elapsed() > timeout {\n",
    "                        to_remove.push(sid.clone());\n",
    "                    }\n",
    "                }\n",
    "            }\n",
    "            for sid in to_remove {\n",
    "                if let Some(arc) = sessions.remove(&sid) {\n",
    "                    expired.push(arc);\n",
    "                }\n",
    "            }\n",
    "        }\n",
    "        let count = expired.len();\n",
    "        for arc in expired {\n",
    "            let mut guard = arc.lock().await;\n",
    "            let _ = guard.close().await;\n",
    "        }\n",
    "        count\n",
    "    }\n",
    "\n",
]

lines[mgr_start:mgr_end] = new_methods

with open(".agents/teamwork/explorer_m4_fix_1/proposed_firecracker.rs", "w") as f:
    f.writelines(lines)

import subprocess
p1 = subprocess.run(["git", "diff", "--no-index", "aura-sdk/aura_sdk/tools/bash.py", ".agents/teamwork/explorer_m4_fix_1/proposed_bash.py"], capture_output=True, text=True)
p2 = subprocess.run(["git", "diff", "--no-index", "aura-core/src/hypervisor/firecracker.rs", ".agents/teamwork/explorer_m4_fix_1/proposed_firecracker.rs"], capture_output=True, text=True)

diff1 = p1.stdout.replace("b/.agents/teamwork/explorer_m4_fix_1/proposed_bash.py", "b/aura-sdk/aura_sdk/tools/bash.py")
diff2 = p2.stdout.replace("b/.agents/teamwork/explorer_m4_fix_1/proposed_firecracker.rs", "b/aura-core/src/hypervisor/firecracker.rs")

with open(".agents/teamwork/explorer_m4_fix_1/remediation.patch", "w") as f:
    f.write(diff1)
    f.write(diff2)

print("Regenerated proposed_firecracker.rs and remediation.patch successfully.")
