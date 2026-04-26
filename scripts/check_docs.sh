#!/usr/bin/env bash
# scripts/check_docs.sh
#
# CI enforcement script for Issue #136: Comprehensive Rustdoc & Security Assertions Sweep.
#
# Fails the pipeline if any `pub fn` or `pub struct` in the Rust source tree
# lacks a preceding `///` doc-comment line.
#
# Usage:
#   ./scripts/check_docs.sh [--fix]
#
# Exit codes:
#   0 – All public items are documented.
#   1 – One or more public items are missing doc comments.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIRS=(
    "$REPO_ROOT/contracts/leaseflow_contracts/src"
    "$REPO_ROOT/crates/leaseflow_math/src"
)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

failures=0
checked=0

echo "🔍 Checking Rustdoc coverage for public items..."
echo ""

for src_dir in "${SRC_DIRS[@]}"; do
    while IFS= read -r -d '' file; do
        # Skip test files — they don't require doc comments.
        if [[ "$file" == *_tests.rs || "$file" == */test.rs ]]; then
            continue
        fi

        line_num=0
        prev_line=""

        while IFS= read -r line; do
            line_num=$((line_num + 1))

            # Match `pub fn` or `pub struct` (not inside a comment or string).
            if echo "$line" | grep -qE '^\s*pub (fn|struct) '; then
                checked=$((checked + 1))
                # The previous non-blank line must be a `///` doc comment.
                if ! echo "$prev_line" | grep -qE '^\s*///'; then
                    echo -e "${RED}MISSING DOC${NC}: $file:$line_num"
                    echo "  → $(echo "$line" | sed 's/^[[:space:]]*//')"
                    failures=$((failures + 1))
                fi
            fi

            # Track previous non-blank line for doc-comment detection.
            if [[ -n "$(echo "$line" | tr -d '[:space:]')" ]]; then
                prev_line="$line"
            fi
        done < "$file"
    done < <(find "$src_dir" -name "*.rs" -print0 2>/dev/null)
done

echo ""
echo "Checked $checked public items."

if [[ $failures -gt 0 ]]; then
    echo -e "${RED}✗ $failures public item(s) are missing doc comments.${NC}"
    echo ""
    echo "Every public fn and struct must have a /// doc comment immediately above it."
    echo "See Issue #136 for the documentation standard."
    exit 1
else
    echo -e "${GREEN}✓ All public items are documented.${NC}"
    exit 0
fi
