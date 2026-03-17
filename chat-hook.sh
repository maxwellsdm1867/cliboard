#!/bin/bash
# Reply hook: pipes the question to claude CLI and sends the response back
# Receives: CLIBOARD_STEP_ID, CLIBOARD_QUESTION, CLIBOARD_CONTEXT as env vars

CLIBOARD="$(dirname "$0")/target/release/cliboard"

# Build prompt with context
PROMPT="You are a concise physics/math tutor answering a question about an equation on a whiteboard. Use \$...\$ for inline LaTeX in your answer. Keep it to 1-3 sentences.

Question: ${CLIBOARD_QUESTION}"

if [ -n "$CLIBOARD_CONTEXT" ] && [ "$CLIBOARD_CONTEXT" != "null" ]; then
    PROMPT="$PROMPT
Context: $CLIBOARD_CONTEXT"
fi

# Call claude CLI in print mode (non-interactive)
ANSWER=$(echo "$PROMPT" | claude --print 2>/dev/null)

if [ -n "$ANSWER" ]; then
    "$CLIBOARD" reply "$CLIBOARD_STEP_ID" "$ANSWER"
fi
