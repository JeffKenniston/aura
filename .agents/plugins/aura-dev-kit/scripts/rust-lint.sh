#!/bin/bash
# Move to workspace root since hooks run in the directory containing hooks.json (.agents)
cd ../../..

# Redirect stdout to stderr so that the only stdout is the JSON response
cargo fmt --check --quiet 1>&2 || cargo check --quiet 1>&2

# Output empty JSON object to satisfy the PostToolUse contract
echo "{}"
