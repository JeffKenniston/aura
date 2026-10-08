use crate::path::FileSystemError;

#[derive(Debug, PartialEq, Eq)]
struct HunkHeader {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum LineOp {
    Context(String),
    Delete(String),
    Add(String),
}

#[derive(Debug)]
struct Hunk {
    header: HunkHeader,
    ops: Vec<LineOp>,
}

fn parse_hunk_header(line: &str) -> Result<HunkHeader, FileSystemError> {
    // Format: @@ -old_start[,old_count] +new_start[,new_count] @@
    let trimmed = line.trim();
    if !trimmed.starts_with("@@") {
        return Err(FileSystemError::PatchError(format!(
            "Expected hunk header starting with '@@', got: '{}'",
            line
        )));
    }

    let end_idx = trimmed[2..]
        .find("@@")
        .ok_or_else(|| FileSystemError::PatchError(format!("Unclosed hunk header: '{}'", line)))?;
    let header_content = trimmed[2..2 + end_idx].trim();
    let parts: Vec<&str> = header_content.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(FileSystemError::PatchError(format!(
            "Malformed hunk header range: '{}'",
            line
        )));
    }

    let parse_range = |s: &str, prefix: char| -> Result<(usize, usize), FileSystemError> {
        let stripped = s.strip_prefix(prefix).ok_or_else(|| {
            FileSystemError::PatchError(format!(
                "Expected prefix '{}' in range '{}'",
                prefix, s
            ))
        })?;
        if let Some((start_s, count_s)) = stripped.split_once(',') {
            let start = start_s.parse::<usize>().map_err(|e| {
                FileSystemError::PatchError(format!("Failed to parse line number: {}", e))
            })?;
            let count = count_s.parse::<usize>().map_err(|e| {
                FileSystemError::PatchError(format!("Failed to parse line count: {}", e))
            })?;
            Ok((start, count))
        } else {
            let start = stripped.parse::<usize>().map_err(|e| {
                FileSystemError::PatchError(format!("Failed to parse line number: {}", e))
            })?;
            Ok((start, 1))
        }
    };

    let (old_start, old_count) = parse_range(parts[0], '-')?;
    let (new_start, new_count) = parse_range(parts[1], '+')?;

    Ok(HunkHeader {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn parse_unified_diff(diff_text: &str) -> Result<Vec<Hunk>, FileSystemError> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<Hunk> = None;

    for raw_line in diff_text.lines() {
        if raw_line.starts_with("--- ") || raw_line.starts_with("+++ ") {
            continue;
        }

        if raw_line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            let header = parse_hunk_header(raw_line)?;
            current_hunk = Some(Hunk {
                header,
                ops: Vec::new(),
            });
            continue;
        }

        if let Some(ref mut hunk) = current_hunk {
            if raw_line.starts_with('\\') {
                // e.g., "\ No newline at end of file"
                continue;
            } else if let Some(content) = raw_line.strip_prefix('+') {
                hunk.ops.push(LineOp::Add(content.to_string()));
            } else if let Some(content) = raw_line.strip_prefix('-') {
                hunk.ops.push(LineOp::Delete(content.to_string()));
            } else if let Some(content) = raw_line.strip_prefix(' ') {
                hunk.ops.push(LineOp::Context(content.to_string()));
            } else if raw_line.is_empty() {
                hunk.ops.push(LineOp::Context(String::new()));
            } else {
                return Err(FileSystemError::PatchError(format!(
                    "Unrecognized diff line format: '{}'",
                    raw_line
                )));
            }
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    if hunks.is_empty() {
        return Err(FileSystemError::PatchError(
            "No valid diff hunks found in provided patch text".to_string(),
        ));
    }

    Ok(hunks)
}

/// Applies a unified diff to the original text, returning the new content (TOOL-FS-001).
pub fn apply_patch(original: &str, diff_text: &str) -> Result<String, FileSystemError> {
    let hunks = parse_unified_diff(diff_text)?;
    let original_lines: Vec<&str> = original.lines().collect();
    let has_trailing_newline = original.ends_with('\n');

    let mut result_lines: Vec<String> = Vec::new();
    let mut orig_cursor = 0; // 0-indexed

    for (hunk_idx, hunk) in hunks.iter().enumerate() {
        // Hunk old_start is 1-indexed (0 means empty file starting)
        let target_start = if hunk.header.old_start == 0 {
            0
        } else {
            hunk.header.old_start - 1
        };

        if target_start < orig_cursor {
            return Err(FileSystemError::PatchError(format!(
                "Hunk #{} target start line {} overlaps with previous hunk cursor {}",
                hunk_idx + 1,
                target_start + 1,
                orig_cursor + 1
            )));
        }

        // Copy untouched lines preceding this hunk
        while orig_cursor < target_start && orig_cursor < original_lines.len() {
            result_lines.push(original_lines[orig_cursor].to_string());
            orig_cursor += 1;
        }

        // Process operations in this hunk
        for op in &hunk.ops {
            match op {
                LineOp::Context(expected) => {
                    if orig_cursor >= original_lines.len() {
                        return Err(FileSystemError::PatchError(format!(
                            "Hunk #{} context mismatch: reached end of file expecting context line '{}'",
                            hunk_idx + 1,
                            expected
                        )));
                    }
                    let actual = original_lines[orig_cursor];
                    if actual != expected {
                        return Err(FileSystemError::PatchError(format!(
                            "Hunk #{} context mismatch at line {}: expected '{}', found '{}'",
                            hunk_idx + 1,
                            orig_cursor + 1,
                            expected,
                            actual
                        )));
                    }
                    result_lines.push(actual.to_string());
                    orig_cursor += 1;
                }
                LineOp::Delete(expected) => {
                    if orig_cursor >= original_lines.len() {
                        return Err(FileSystemError::PatchError(format!(
                            "Hunk #{} delete mismatch: reached end of file expecting deletion of line '{}'",
                            hunk_idx + 1,
                            expected
                        )));
                    }
                    let actual = original_lines[orig_cursor];
                    if actual != expected {
                        return Err(FileSystemError::PatchError(format!(
                            "Hunk #{} delete mismatch at line {}: expected '{}', found '{}'",
                            hunk_idx + 1,
                            orig_cursor + 1,
                            expected,
                            actual
                        )));
                    }
                    // Delete: simply advance orig_cursor without appending to result_lines
                    orig_cursor += 1;
                }
                LineOp::Add(added) => {
                    result_lines.push(added.clone());
                }
            }
        }
    }

    // Append remaining untouched lines after the last hunk
    while orig_cursor < original_lines.len() {
        result_lines.push(original_lines[orig_cursor].to_string());
        orig_cursor += 1;
    }

    let mut output = result_lines.join("\n");
    if has_trailing_newline || !result_lines.is_empty() {
        output.push('\n');
    }

    Ok(output)
}
