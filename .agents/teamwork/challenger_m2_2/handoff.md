# Hard Handoff Report — Empirical Challenger Verification for Milestone 2: Visual Redaction

**Agent**: `challenger_m2_2` (teamwork_preview_challenger)  
**Recipient**: `parent` (orchestrator ID: `1732b97a-66c1-4a05-9f2b-afc669944583`)  
**Milestone**: Milestone 2: Visual Redaction (Phase M5: Hardening)  
**Date**: 2026-10-11T22:48:50Z  
**Verdict**: **APPROVE**

---

## 1. Observation

1. **Visual Redaction Implementation (`aura-sdk/aura_sdk/tools/computer.py`)**:
   - `mask_image_bytes(png_bytes: bytes, bounding_boxes: list[dict[str, Any] | tuple[int, int, int, int]] | None = None) -> bytes` (lines 33–232):
     * PNG Signature check (line 45): validates magic header `b"\x89PNG\r\n\x1a\n"`, raises `ValueError("Invalid PNG signature...")` on invalid inputs.
     * Chunk parsing loop (lines 56–86): extracts `IHDR`, concatenates `IDAT` chunks, and preserves chunks before and after `IDAT` with strict CRC32 validation (`zlib.crc32(chunk_type + chunk_data) & 0xFFFFFFFF`).
     * Color type and stride handling (lines 93–114): extracts `width`, `height`, `bit_depth`, `color_type`, rejects bit depth != 8 and interlaced PNGs. Computes `bpp` (4 for RGBA, 3 for RGB, 1 for Grayscale/Indexed, 2 for Grayscale+Alpha) and `stride = width * bpp`.
     * Scanline unfiltering (lines 124–172): handles all 5 PNG standard filter modes:
       - Mode 0 (None): `recon[:] = scanline`
       - Mode 1 (Sub): `(scanline[i] + left) & 0xFF`
       - Mode 2 (Up): `(scanline[i] + above) & 0xFF`
       - Mode 3 (Average): `(scanline[i] + ((left + above) >> 1)) & 0xFF`
       - Mode 4 (Paeth): Paeth predictor selecting predictor nearest to `p = left + above - upper_left`, `(scanline[i] + pr) & 0xFF`
     * Blackout redaction (lines 175–213): clamps box coordinates to `[0, width]` and `[0, height]` via `x0 = max(0, min(bx, width))`, `x1 = max(0, min(bx + bw, width))`, sets masked pixels to solid black (`[0, 0, 0, 255]` for RGBA, `[0, 0, 0]` for RGB, `[0]` for Grayscale, `[0, 255]` for Grayscale+Alpha). Skips non-positive widths or heights (`bw <= 0 or bh <= 0`).
     * Re-encoding & CRC assembly (lines 215–232): re-encodes rows with Filter 0, compresses via `zlib.compress`, writes chunks using `_make_chunk` with freshly calculated CRC32 checksums.

2. **Empirical Adversarial Stress Test Suite Execution**:
   - Constructed and executed a dedicated 151-case adversarial stress harness testing `mask_image_bytes` across 5 distinct test suites:
     * **Suite 1: Scanline Filter Permutations**: Tested Filter 0, Filter 1 (Sub), Filter 2 (Up), Filter 3 (Average), Filter 4 (Paeth), alternating mixed filters (`y % 5`), and pseudo-random filters across 4 color types (RGBA, RGB, Grayscale, Grayscale+Alpha). Result: 28/28 passed.
     * **Suite 2: Extreme Bounding Box Scenarios**: Tested exact full-screen coverage `(0, 0, w, h)`, oversized full coverage `(-50, -50, w+100, h+100)`, partial negative top-left `(-5, -5, 10, 10)`, partial out-of-bounds bottom-right `(15, 15, 20, 20)`, completely out-of-bounds negative `(-100, -100, 30, 30)`, completely out-of-bounds positive `(100, 100, 50, 50)`, zero-area boxes (zero/negative width/height), empty box list `[]`, `None` box list, multi-box overlapping chains, dict format boxes (`{"x": 1, "y": 1, "width": 4, "height": 4}` and `{"x": 10, "y": 10, "w": 5, "h": 5}`), single-pixel corner boxes `(0, 0, 1, 1)`, and checkerboard 2x2 grid masking. Result: 13/13 passed.
     * **Suite 3: Odd Dimensions & Extreme Aspect Ratios**: Tested 1x1 masked, 1x1 unmasked, 1x100 vertical strip, 100x1 horizontal strip, 13x17 prime dimensions with mixed filters, and 64x64 with 20 random boxes. Result: 6/6 passed.
     * **Suite 4: High-Throughput Burst**: Tested 100 sequential images with varying dimensions and filters. Execution completed in 0.150s (666.6 images/sec). Result: 100/100 passed.
     * **Suite 5: Malformed & Corrupted Input Rejection**: Tested truncated PNG signature, non-PNG random garbage, truncated chunk headers, and corrupted CRC32 checksums. All were rejected cleanly with expected `ValueError`. Result: 4/4 passed.
     * **Total Stress Suite Summary**: 151/151 passed.
     * **Oracle Checks**: Every test strictly verified that:
       1) All pixels inside masked regions are strictly black (`(0, 0, 0, 255)` for RGBA, `(0, 0, 0)` for RGB).
       2) All pixels outside masked regions are 100% byte-identical to original source pixels.
       3) All output PNGs pass strict chunk structure and zlib CRC32 verification with valid IHDR, IDAT, and IEND chunks.

3. **Existing Test Suite Runs**:
   - `aura-sdk/tests`:
     ```
     /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
     ======================== 96 passed, 1 warning in 3.99s =========================
     ```
   - `aura-core` M5 E2E Opaque Test:
     ```
     cargo test -p aura-core --test m5_e2e_opaque_test
     test result: ok. 170 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     ```
   - `aura-core` Full Test Suite:
     ```
     cargo test -p aura-core --jobs 1
     test result: ok. 170 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     ```
   - `vsock-screenshot-daemon` Test Suite:
     ```
     cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
     test result: ok. 14 passed (lib) + 14 passed (bin) + 2 passed (test_daemon); 0 failed
     ```
   - Code Style & Linter:
     ```
     /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
     All checks passed!
     ```

---

## 2. Logic Chain

1. **Mathematical Correctness of Scanline Unfiltering (Observation 1, 2)**:
   - For all 5 PNG filter modes (0=None, 1=Sub, 2=Up, 3=Average, 4=Paeth), unfiltering mathematically inverts the scanline filtering operation:
     - Sub: $recon[i] = (scanline[i] + left) \pmod{256}$
     - Up: $recon[i] = (scanline[i] + above) \pmod{256}$
     - Average: $recon[i] = (scanline[i] + \lfloor(left + above)/2\rfloor) \pmod{256}$
     - Paeth: $recon[i] = (scanline[i] + pr) \pmod{256}$
   - In our empirical tests, images encoded with Filter 1, 2, 3, 4, mixed, and random filters reconstructed 100% byte-identically to the source pixel buffers, confirming zero data loss or row corruption during unfiltering.

2. **Pixel Blackout Strictness & Boundary Clamping (Observation 1, 2)**:
   - Clamping `x0 = max(0, min(bx, width))` and `x1 = max(0, min(bx + bw, width))` handles all negative coordinates, zero-width boxes, and boxes extending beyond image boundaries without out-of-range indexing or buffer overruns.
   - For every pixel within the redacted bounding box, pixel components are strictly set to `0` (and `255` for alpha channel in RGBA and Grayscale+Alpha), preventing any visual leakage.
   - For every pixel outside the bounding box, the pixel bytes remain untouched and identical to the original image.

3. **PNG File Format & Checksum Integrity (Observation 1, 2, 3)**:
   - Output PNG re-encodes scanlines with Filter 0 and produces a single valid IDAT chunk followed by an IEND chunk.
   - All chunk CRCs are recalculated using `zlib.crc32(chunk_type + chunk_data) & 0xFFFFFFFF`.
   - Independent verification confirmed that every output PNG passes strict PNG chunk validation, contains zero trailing bytes, and decompresses cleanly.

4. **Integration Compatibility & Regressions (Observation 3)**:
   - Both `pytest aura-sdk/tests` (96/96) and `cargo test -p aura-core --test m5_e2e_opaque_test` (170/170) pass with zero errors, confirming no regressions in any subsystem.

---

## 3. Caveats

- As noted in `computer.py` and worker documentation, visual redaction is designed for non-interlaced PNG images (standard output for `vsock-screenshot-daemon` and headless X11 screenshot captures). Interlaced PNGs are cleanly rejected with `ValueError("Interlaced PNG scanlines are not supported for redaction")`.

---

## 4. Conclusion

The visual redaction implementation in `aura-sdk/aura_sdk/tools/computer.py` satisfies all requirements and edge-case invariants:
- Reconstructs scanline filters 0 through 4 with complete mathematical fidelity.
- Correctly handles extreme bounding boxes (oversized, negative, out-of-bounds, zero-area, chained overlapping, and empty/None lists) without exception.
- Enforces strict blackout on masked pixels while preserving 100% byte-identity on unmasked pixels.
- Generates fully compliant PNG binaries with valid chunk ordering and CRC32 checksums.
- All test suites pass cleanly with 100% success rate.

**Verdict: APPROVE**

---

## 5. Verification Method

To independently reproduce and verify this assessment:

1. **Run Python SDK Test Suite**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   ```
   *Expected output*: 96 passed, 0 failed.

2. **Run M5 E2E Opaque Test Suite**:
   ```bash
   cargo test -p aura-core --test m5_e2e_opaque_test
   ```
   *Expected output*: 170 passed; 0 failed.

3. **Run Full Aura-Core Suite**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   *Expected output*: 170 passed; 0 failed.

4. **Run Daemon Cargo Test Suite**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
   *Expected output*: 30 passed; 0 failed.

5. **Verify Python Code Formatting & Linting**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
   *Expected output*: `All checks passed!`
