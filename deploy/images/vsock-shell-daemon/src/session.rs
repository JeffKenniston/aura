use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::io::AsRawFd;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

/// Output of a bash command executed in a session (TOOL-BASH-002).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BashCommandOutput {
    pub session_id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: Option<String>,
}

/// A resident interactive shell session wrapping a persistent `/bin/bash` process.
pub struct ShellSession {
    child: Child,
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
    current_cwd: String,
}

impl ShellSession {
    /// Spawns a new resident `/bin/bash` process with piped I/O.
    pub fn new() -> Result<Self, std::io::Error> {
        let mut child = Command::new("bash")
            .arg("--noprofile")
            .arg("--norc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture child stdin")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture child stdout")
        })?;

        // Configure child stderr as non-blocking so reads never block when no output is produced
        if let Some(ref stderr) = child.stderr {
            let fd = stderr.as_raw_fd();
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL, 0);
                if flags >= 0 {
                    let _ = libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
        }

        let stdout_reader = BufReader::new(stdout);
        let current_cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/root"))
            .to_string_lossy()
            .to_string();

        Ok(Self {
            child,
            stdin,
            stdout_reader,
            current_cwd,
        })
    }

    /// Checks if the child process has terminated, respawning it if necessary.
    fn ensure_alive(&mut self) -> Result<(), std::io::Error> {
        let has_exited = match self.child.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(_) => true,
        };

        if has_exited {
            let mut fresh = Self::new()?;
            // Restore previous working directory in fresh shell
            let restore_cmd = format!("cd \"{}\" 2>/dev/null || true\n", self.current_cwd);
            let _ = fresh.stdin.write_all(restore_cmd.as_bytes());
            let _ = fresh.stdin.flush();
            *self = fresh;
        }

        Ok(())
    }

    /// Executes a command in the resident bash process and captures its output.
    pub fn execute(
        &mut self,
        session_id: &str,
        command: &str,
        _timeout: Duration,
    ) -> Result<BashCommandOutput, std::io::Error> {
        self.ensure_alive()?;

        let marker = uuid::Uuid::new_v4().simple().to_string();
        let sentinel_cwd = format!("__AURA_CWD__{}", marker);
        let sentinel_exit = format!("__AURA_EXIT__{}", marker);

        // Inject command followed by exit code capture and sentinel markers
        let script = format!(
            "{}\n__AURA_STATUS__=$?\necho \"\n{}:$PWD\"\necho \"{}:$__AURA_STATUS__\"\n",
            command, sentinel_cwd, sentinel_exit
        );

        if let Err(_e) = self.stdin.write_all(script.as_bytes()) {
            // Broken pipe, try respawning and re-executing once
            self.ensure_alive()?;
            self.stdin.write_all(script.as_bytes())?;
        }
        self.stdin.flush()?;

        let mut captured_lines = Vec::new();
        let mut exit_code = 0;
        let mut reported_cwd = None;

        let mut line = String::new();
        while self.stdout_reader.read_line(&mut line)? > 0 {
            let trimmed = line.trim_end();
            if trimmed.starts_with(&sentinel_exit) {
                if let Some(code_str) = trimmed.strip_prefix(&format!("{}:", sentinel_exit)) {
                    exit_code = code_str.parse::<i32>().unwrap_or(0);
                }
                break;
            } else if trimmed.starts_with(&sentinel_cwd) {
                if let Some(cwd_str) = trimmed.strip_prefix(&format!("{}:", sentinel_cwd)) {
                    reported_cwd = Some(cwd_str.to_string());
                    self.current_cwd = cwd_str.to_string();
                }
            } else {
                captured_lines.push(line.clone());
            }
            line.clear();
        }

        // Drain any available stderr non-blockingly
        let mut stderr_bytes = Vec::new();
        if let Some(ref mut stderr) = self.child.stderr {
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => stderr_bytes.extend_from_slice(&buf[..n]),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
        }

        let stdout_result = captured_lines.join("").trim_end().to_string();
        let stderr_result = String::from_utf8_lossy(&stderr_bytes).trim_end().to_string();

        Ok(BashCommandOutput {
            session_id: session_id.to_string(),
            stdout: stdout_result,
            stderr: stderr_result,
            exit_code,
            cwd: reported_cwd.or_else(|| Some(self.current_cwd.clone())),
        })
    }
}

impl Drop for ShellSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Request sent to the shell daemon (TOOL-BASH-002).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashSessionRequest {
    pub session_id: String,
    pub command: String,
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_action() -> String {
    "execute".to_string()
}

/// In-memory manager for multiple shell sessions.
pub struct SessionManager {
    sessions: HashMap<String, ShellSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn execute(&mut self, req: &BashSessionRequest) -> Result<BashCommandOutput, std::io::Error> {
        match req.action.as_str() {
            "close" => {
                self.sessions.remove(&req.session_id);
                Ok(BashCommandOutput {
                    session_id: req.session_id.clone(),
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                    cwd: None,
                })
            }
            "ping" => Ok(BashCommandOutput {
                session_id: req.session_id.clone(),
                stdout: "pong".to_string(),
                stderr: String::new(),
                exit_code: 0,
                cwd: None,
            }),
            _ => {
                let session = match self.sessions.get_mut(&req.session_id) {
                    Some(s) => s,
                    None => {
                        let new_s = ShellSession::new()?;
                        self.sessions.insert(req.session_id.clone(), new_s);
                        self.sessions.get_mut(&req.session_id).unwrap()
                    }
                };
                session.execute(&req.session_id, &req.command, Duration::from_secs(125))
            }
        }
    }

    #[allow(dead_code)]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_session_basic_execution() {
        let mut session = ShellSession::new().expect("Failed to spawn shell");
        let output = session
            .execute("test-sid", "echo 'Hello Aura'", Duration::from_secs(5))
            .expect("Execution failed");
        assert_eq!(output.stdout, "Hello Aura");
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.session_id, "test-sid");
    }

    #[test]
    fn test_shell_session_environment_variable_continuity() {
        let mut session = ShellSession::new().expect("Failed to spawn shell");
        let sid = "env-continuity-test";

        let out1 = session
            .execute(sid, "export AURA_SESSION_KEY=AlphaOmega42", Duration::from_secs(5))
            .expect("Export failed");
        assert_eq!(out1.exit_code, 0);

        let out2 = session
            .execute(sid, "echo $AURA_SESSION_KEY", Duration::from_secs(5))
            .expect("Echo failed");
        assert_eq!(out2.stdout, "AlphaOmega42");
        assert_eq!(out2.exit_code, 0);
    }

    #[test]
    fn test_shell_session_working_directory_continuity() {
        let mut session = ShellSession::new().expect("Failed to spawn shell");
        let sid = "cwd-continuity-test";

        let out1 = session
            .execute(sid, "cd /tmp", Duration::from_secs(5))
            .expect("cd failed");
        assert_eq!(out1.exit_code, 0);

        let out2 = session
            .execute(sid, "pwd", Duration::from_secs(5))
            .expect("pwd failed");
        let real_tmp = std::fs::canonicalize("/tmp").unwrap().to_string_lossy().to_string();
        assert!(out2.stdout == "/tmp" || out2.stdout == real_tmp);
        assert_eq!(out2.exit_code, 0);
    }

    #[test]
    fn test_shell_session_non_zero_exit_code() {
        let mut session = ShellSession::new().expect("Failed to spawn shell");
        let sid = "exit-code-test";

        let out = session
            .execute(sid, "ls /nonexistent_aura_test_dir_9999", Duration::from_secs(5))
            .expect("Execution failed");
        assert_ne!(out.exit_code, 0);
        assert!(!out.stderr.is_empty() || !out.stdout.is_empty());
    }

    #[test]
    fn test_session_manager_isolation() {
        let mut manager = SessionManager::new();

        let req_a1 = BashSessionRequest {
            session_id: "session-a".to_string(),
            command: "export SHARED_VAR=FromA".to_string(),
            action: "execute".to_string(),
        };
        let _ = manager.execute(&req_a1).unwrap();

        let req_b = BashSessionRequest {
            session_id: "session-b".to_string(),
            command: "echo $SHARED_VAR".to_string(),
            action: "execute".to_string(),
        };
        let out_b = manager.execute(&req_b).unwrap();
        assert_eq!(out_b.stdout, "");

        let req_a2 = BashSessionRequest {
            session_id: "session-a".to_string(),
            command: "echo $SHARED_VAR".to_string(),
            action: "execute".to_string(),
        };
        let out_a = manager.execute(&req_a2).unwrap();
        assert_eq!(out_a.stdout, "FromA");
    }

    #[test]
    fn test_session_manager_close_and_ping() {
        let mut manager = SessionManager::new();
        let ping_req = BashSessionRequest {
            session_id: "pinger".to_string(),
            command: "".to_string(),
            action: "ping".to_string(),
        };
        let ping_out = manager.execute(&ping_req).unwrap();
        assert_eq!(ping_out.stdout, "pong");

        let exec_req = BashSessionRequest {
            session_id: "temp".to_string(),
            command: "echo 123".to_string(),
            action: "execute".to_string(),
        };
        let _ = manager.execute(&exec_req).unwrap();
        assert_eq!(manager.session_count(), 1);

        let close_req = BashSessionRequest {
            session_id: "temp".to_string(),
            command: "".to_string(),
            action: "close".to_string(),
        };
        let _ = manager.execute(&close_req).unwrap();
        assert_eq!(manager.session_count(), 0);
    }
}
