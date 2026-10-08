use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::patch::apply_patch;
use crate::path::PathValidator;

/// Production filesystem service enforcing atomic file operations (TOOL-FS-001)
/// and strict path-prefix boundaries (TOOL-FS-002).
#[derive(Clone, Debug)]
pub struct FileSystemService {
    validator: PathValidator,
}

impl Default for FileSystemService {
    fn default() -> Self {
        Self::new(PathValidator::from_env_or_default().workspace_root())
    }
}

impl FileSystemService {
    /// Constructs a `FileSystemService` scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            validator: PathValidator::new(workspace_root),
        }
    }

    /// Returns the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        self.validator.workspace_root()
    }

    /// Reads a file atomically within the authorized workspace (TOOL-FS-001, TOOL-FS-002).
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        let validated_path = self
            .validator
            .resolve_and_validate(path)
            .map_err(|e| e.to_string())?;

        let mut file = File::open(&validated_path).map_err(|e| {
            format!(
                "Failed to open file '{}': {}",
                validated_path.display(),
                e
            )
        })?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| {
            format!(
                "Failed to read file '{}': {}",
                validated_path.display(),
                e
            )
        })?;

        Ok(buffer)
    }

    /// Writes a file atomically within the authorized workspace (TOOL-FS-001, TOOL-FS-002).
    /// Prevents corrupted/partial writes by writing to an adjacent tempfile and atomically renaming.
    pub fn write_file(&self, path: &str, contents: &[u8]) -> Result<(), String> {
        let validated_path = self
            .validator
            .resolve_and_validate(path)
            .map_err(|e| e.to_string())?;

        let parent_dir = validated_path.parent().ok_or_else(|| {
            format!(
                "Invalid path '{}': no parent directory found",
                validated_path.display()
            )
        })?;

        // Ensure parent directory hierarchy exists
        fs::create_dir_all(parent_dir).map_err(|e| {
            format!(
                "Failed to create parent directory '{}': {}",
                parent_dir.display(),
                e
            )
        })?;

        // Generate temporary file in the exact same directory (guarantees same filesystem mount for atomic rename)
        let file_stem = validated_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "atomic_write".to_string());
        let temp_filename = format!(".tmp.{}.{}", Uuid::new_v4(), file_stem);
        let temp_path = parent_dir.join(temp_filename);

        // Write and sync to disk
        let write_result = (|| -> Result<(), std::io::Error> {
            let mut temp_file = File::create(&temp_path)?;
            temp_file.write_all(contents)?;
            temp_file.flush()?;
            temp_file.sync_all()?;
            Ok(())
        })();

        if let Err(e) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "Failed writing temporary file '{}': {}",
                temp_path.display(),
                e
            ));
        }

        // Perform atomic rename replacing target destination
        if let Err(e) = fs::rename(&temp_path, &validated_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "Atomic rename from '{}' to '{}' failed: {}",
                temp_path.display(),
                validated_path.display(),
                e
            ));
        }

        Ok(())
    }

    /// Patches an existing file atomically using a unified diff format (TOOL-FS-001, TOOL-FS-002).
    /// Returns the applied diff text on success.
    pub fn patch_file(&self, path: &str, diff: &str) -> Result<String, String> {
        let validated_path = self
            .validator
            .resolve_and_validate(path)
            .map_err(|e| e.to_string())?;

        // Read current content if file exists, or start from empty
        let current_content = if validated_path.exists() {
            fs::read_to_string(&validated_path).map_err(|e| {
                format!(
                    "Failed to read file '{}' for patching: {}",
                    validated_path.display(),
                    e
                )
            })?
        } else {
            String::new()
        };

        // Apply unified patch
        let patched_content = apply_patch(&current_content, diff)
            .map_err(|e| format!("Patch application error on '{}': {}", path, e))?;

        // Atomically write the updated contents
        self.write_file(path, patched_content.as_bytes())?;

        Ok(diff.to_string())
    }
}
