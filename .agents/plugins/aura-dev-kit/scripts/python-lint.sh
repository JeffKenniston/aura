#!/bin/bash
# Move to workspace root since hooks run in the directory containing hooks.json (.agents)
cd ../../..

# Redirect stdout to stderr so that the only stdout is the JSON response
ruff check --quiet . 1>&2 && ruff format --check --quiet . 1>&2

# Output empty JSON object to satisfy the PostToolUse contract
echo "{}"
