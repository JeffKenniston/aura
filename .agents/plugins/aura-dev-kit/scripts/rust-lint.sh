#!/bin/bash
# Parse the hook input from stdin
INPUT=$(cat)
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcriptPath')
STEP_IDX=$(echo "$INPUT" | jq -r '.stepIdx')

# Extract TargetFile arguments from the given step in the transcript
# and check if any end with .rs
RS_MODIFIED=$(jq -r "select(.step_index == ${STEP_IDX} and .type == \"PLANNER_RESPONSE\") | .tool_calls[]? | .args.TargetFile? | select(. != null)" "$TRANSCRIPT" | grep "\.rs$")

if [ -n "$RS_MODIFIED" ]; then
    cd ../../..
    cargo fmt --check --quiet 1>&2 || cargo check --quiet 1>&2
fi

# Always output an empty JSON object to satisfy the PostToolUse contract
echo "{}"
