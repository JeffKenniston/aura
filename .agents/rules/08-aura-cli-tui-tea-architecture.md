---
title: Aura CLI Terminal UI and The Elm Architecture (TEA)
description: Specifies immediate-mode double-buffering, tokio::select! event multiplexing, Sixel/Kitty rendering, and headless -p mode.
tags: [aura-cli, ratatui, crossterm, tea, tui, headless]
version: 1.0
---

# Aura CLI Terminal UI & The Elm Architecture (TEA)

## 1. Stateless Presentation Layer
- `aura-cli` must remain strictly a presentation node and IPC client.
- Direct LLM HTTP network stacks, database drivers, and microVM management are strictly prohibited in `aura-cli`. All such actions are delegated to `aura-core`.

## 2. Asynchronous The Elm Architecture (TEA) Event Loop
- State model (`AppModel`) is immutable during rendering.
- The central event loop runs in a `tokio::select!` block, multiplexing three non-blocking asynchronous streams:
  1. **Terminal Stream (`crossterm`)**: User keystrokes, window resize events, mouse interactions.
  2. **Zenoh Event Bus (`zenoh-shm`)**: Microkernel state changes, tool execution chunks, and model thoughts.
  3. **WASI 0.3 Background Channel (`mpsc`)**: Asynchronous local tool telemetry and token counter updates.

## 3. Immediate-Mode Double Buffering & Graphics
- Rendering runs at up to 60 FPS using `ratatui`'s double-buffer engine.
- Only delta coordinates are emitted to stdout to eliminate screen tearing during high-frequency token streams.
- Inline visual artifacts (browser screenshots, GUI recordings) are rendered using `ratatui-image` via Sixel, Kitty Graphics Protocol, or iTerm2 sequences.

## 4. Modal Vim Input Buffer
- Pressing `v` in normal mode opens a modal Vim buffer for composing multi-line prompts and detailed architectural instructions.

## 5. Headless Automation Mode (`--headless -p`)
- When invoked with `--headless -p "<prompt>"`, the TUI screen buffer and raw mode hooks are suppressed.
- The CLI hashes `AGENTS.md` with BLAKE3 to synchronize constitutional invariants with `aura-core`.
- Output streams as structured JSON or SSE directly to stdout for CI/CD and subagent scripting.
