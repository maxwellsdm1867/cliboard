#!/bin/bash
# Reply hook: pipes the question to claude CLI and sends the response back
# Receives: CLIBOARD_STEP_ID, CLIBOARD_QUESTION, CLIBOARD_CONTEXT as env vars

CLIBOARD="$(dirname "$0")/target/release/cliboard"

# Build prompt with context using a temp file to avoid shell injection
PROMPT_FILE="$(mktemp)" || exit 1
trap 'rm -f "$PROMPT_FILE"' EXIT

printf '%s\n' 'You are a concise physics/math tutor answering a question about an equation on a whiteboard. Use $...$ for inline LaTeX in your answer. Keep it to 1-3 sentences.' > "$PROMPT_FILE"
printf '\nQuestion: %s\n' "$CLIBOARD_QUESTION" >> "$PROMPT_FILE"

if [ -n "$CLIBOARD_CONTEXT" ] && [ "$CLIBOARD_CONTEXT" != "null" ]; then
    printf 'Context: %s\n' "$CLIBOARD_CONTEXT" >> "$PROMPT_FILE"
fi

# Call claude CLI in print mode (non-interactive)
ANSWER=$(claude --print < "$PROMPT_FILE" 2>/dev/null)

if [ -n "$ANSWER" ]; then
    "$CLIBOARD" reply "$CLIBOARD_STEP_ID" "$ANSWER"
fi
