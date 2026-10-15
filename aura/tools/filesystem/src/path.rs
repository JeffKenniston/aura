use std::path::{Component, Path, PathBuf};
use std::fmt;

/// Errors that can occur during filesystem operations or path validation.
#[derive(Debug)]
pub enum FileSystemError {
    /// Path traversal was detected (e.g., prohibited '..' segments).
    PathTraversalDenied(String),
    /// Target path falls outside the authorized workspace boundary (TOOL-FS-002).
    OutsideWorkspaceBoundary { path: PathBuf, root: PathBuf },
    /// Invalid or malformed path provided.
    InvalidPath(String),
    /// Standard I/O error occurred.
    Io(std::io::Error),
    /// Patch conflict or syntax mismatch (TOOL-FS-001).
    PatchError(String),
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathTraversalDenied(msg) => write!(f, "Path traversal denied: {}", msg),
            Self::OutsideWorkspaceBoundary { path, root } => {
                write!(
                    f,
                    "Path '{}' is outside workspace boundary '{}'",
                    path.display(),
                    root.display()
                )
            }
            Self::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            Self::Io(err) => write!(f, "Filesystem I/O error: {}", err),
            Self::PatchError(msg) => write!(f, "Patch failure: {}", msg),
        }
    }
}

impl std::error::Error for FileSystemError {}

impl From<std::io::Error> for FileSystemError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Enforces strict path-prefix boundaries on all filesystem operations (TOOL-FS-002).
#[derive(Clone, Debug)]
pub struct PathValidator {
    workspace_root: PathBuf,
}

impl PathValidator {
    /// Creates a new `PathValidator` with the given workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let root = workspace_root.into();
        let canonical_root = root.canonicalize().unwrap_or(root);
        Self {
            workspace_root: canonical_root,
        }
    }

    /// Creates a `PathValidator` from the `AURA_WORKSPACE_ROOT` environment variable,
    /// falling back to the current directory or `/home/jeff/aura`.
    pub fn from_env_or_default() -> Self {
        let root_str = std::env::var("AURA_WORKSPACE_ROOT")
            .unwrap_or_else(|_| "/home/jeff/aura".to_string());
        let root_path = PathBuf::from(root_str);
        let effective_path = if root_path.exists() {
            root_path
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        Self::new(effective_path)
    }

    /// Returns the canonical workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Resolves and validates a target path, ensuring it stays strictly inside the workspace.
    /// Traps/errors if path traversal or escape is detected (TOOL-FS-002).
    pub fn resolve_and_validate(&self, input_path: &str) -> Result<PathBuf, FileSystemError> {
        let trimmed = input_path.trim();
        if trimmed.is_empty() {
            return Err(FileSystemError::InvalidPath("Target path cannot be empty".to_string()));
        }

        let raw_path = Path::new(trimmed);

        // 1. Prohibit relative parent navigation ('..') anywhere in components (Zero-Trust Guard)
        for comp in raw_path.components() {
            if comp == Component::ParentDir {
                return Err(FileSystemError::PathTraversalDenied(format!(
                    "Path '{}' contains prohibited parent traversal '..' segment",
                    input_path
                )));
            }
        }

        // 2. Resolve target against workspace root
        let candidate_path = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            self.workspace_root.join(raw_path)
        };

        // 3. Boundary validation
        if candidate_path.exists() {
            // Target exists: canonicalize and verify prefix
            let canonical_target = candidate_path.canonicalize().map_err(FileSystemError::Io)?;
            if !canonical_target.starts_with(&self.workspace_root) {
                return Err(FileSystemError::OutsideWorkspaceBoundary {
                    path: canonical_target,
                    root: self.workspace_root.clone(),
                });
            }
            Ok(canonical_target)
        } else {
            // Target does not exist yet (e.g., new file to write):
            // Traverse upward to the lowest existing parent directory and canonicalize it
            let mut curr = candidate_path.as_path();
            let mut non_existing_parts = Vec::new();

            while !curr.exists() {
                if let Some(file_name) = curr.file_name() {
                    non_existing_parts.push(file_name);
                }
                match curr.parent() {
                    Some(parent) => curr = parent,
                    None => break,
                }
            }

            let canonical_existing_parent = if curr.exists() {
                curr.canonicalize().map_err(FileSystemError::Io)?
            } else {
                self.workspace_root.clone()
            };

            // Verify existing ancestor stays within workspace root
            if !canonical_existing_parent.starts_with(&self.workspace_root) {
                return Err(FileSystemError::OutsideWorkspaceBoundary {
                    path: candidate_path,
                    root: self.workspace_root.clone(),
                });
            }

            // Reconstruct verified path
            let mut final_path = canonical_existing_parent;
            for part in non_existing_parts.into_iter().rev() {
                final_path.push(part);
            }

            if !final_path.starts_with(&self.workspace_root) {
                return Err(FileSystemError::OutsideWorkspaceBoundary {
                    path: final_path,
                    root: self.workspace_root.clone(),
                });
            }

            Ok(final_path)
        }
    }
}
