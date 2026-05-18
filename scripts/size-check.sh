#!/usr/bin/env bash
# scripts/size-check.sh
#
# File-size gate. For each .rs/.ts/.tsx file changed in the configured
# diff range, refuse the push when the file exceeds MAX_LINES (default
# 600). Designed to nudge SRP/SOLID — long files almost always do too
# much. Test files and non-code files are ignored.
#
# Usage:
#   scripts/size-check.sh                       # diff: HEAD~1..HEAD
#   BASE_REF=origin/main scripts/size-check.sh  # diff: origin/main...HEAD
#   MODE=report scripts/size-check.sh           # never exit non-zero
#   MAX_LINES=800 scripts/size-check.sh         # raise the threshold
#
# Files with `// size:exclude file` on line 1 are reported as EXCLUDED
# and don't count against the gate. Reserve for files that are
# genuinely structural (huge generated tables, long but cohesive parser
# state machines), document the exception in tech-debt.md.
#
# Exit codes:
#   0  every touched code file is within the limit (or no code files)
#   1  at least one is over and MODE != report
#   2  setup error

set -euo pipefail
export LC_ALL=C
export LC_NUMERIC=C

MAX_LINES="${MAX_LINES:-600}"
MODE="${MODE:-enforce}"
BASE_REF="${BASE_REF:-}"

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

# ---------- changed file set --------------------------------------------------

DIFF_MODE="${DIFF_MODE:-}"

if [ "$DIFF_MODE" = "staged" ]; then
    DIFF_CMD=(git diff --cached --name-only)
else
    if [ -n "$BASE_REF" ]; then
        DIFF_RANGE="$BASE_REF...HEAD"
    else
        DIFF_RANGE="HEAD~1..HEAD"
    fi

    if ! git rev-parse --verify HEAD~1 >/dev/null 2>&1 && [ -z "$BASE_REF" ]; then
        echo "size-check: only one commit in branch; nothing to check"
        exit 0
    fi
    DIFF_CMD=(git diff --name-only "$DIFF_RANGE")
fi

CHANGED_FILES=()
while IFS= read -r line; do
    [ -n "$line" ] && CHANGED_FILES+=("$line")
done < <("${DIFF_CMD[@]}" 2>/dev/null \
    | grep -E '\.(rs|ts|tsx)$' \
    | grep -v -E '/(__tests__|tests)/' \
    | grep -v -E '\.(test|spec|browser\.test|browser\.spec)\.(ts|tsx)$' \
    | grep -v -E '/test/' \
    | grep -v -E '\.d\.ts$' \
    | grep -v -E '(^|/)(vite|vitest|playwright|tsup|tsconfig)\.config\.(ts|tsx)$' \
    || true)

# Bash 3 (default on macOS) under `set -u` errors on `${empty_array[@]}`.
# Skip early when there's nothing to check.
if [ ${#CHANGED_FILES[@]} -eq 0 ]; then
    echo "size-check: no .rs/.ts/.tsx changes; gate skipped"
    exit 0
fi

# Filter to existing files (rename + delete leave entries that no longer exist)
KEPT=()
for f in "${CHANGED_FILES[@]}"; do
    [ -f "$f" ] && KEPT+=("$f")
done

if [ ${#KEPT[@]} -eq 0 ]; then
    echo "size-check: all touched files were deleted; gate skipped"
    exit 0
fi
CHANGED_FILES=("${KEPT[@]}")

# ---------- evaluate ---------------------------------------------------------

printf "\n%-70s  %-7s  %-7s\n" "FILE" "LINES" "STATUS"
printf "%-70s  %-7s  %-7s\n" "----" "-----" "------"

# effective_lines <file>
# Counts production lines only. For Rust, the inline `mod tests { ... }`
# block is excluded — Rust idioms put tests right next to the code, and
# tests legitimately add bulk that says nothing about SRP. For
# TypeScript/TSX, tests live in separate `.test.ts(x)` files (already
# filtered upstream), so the whole file counts.
effective_lines() {
    local f="$1"
    case "$f" in
        *.rs)
            awk '/^[[:space:]]*mod tests([[:space:]]*\{|[[:space:]]*$)/ { exit } { count++ } END { print count+0 }' "$f"
            ;;
        *)
            wc -l < "$f" | tr -d ' '
            ;;
    esac
}

FAILED=0
OVERSIZED_FILES=()

for f in "${CHANGED_FILES[@]}"; do
    if head -n 10 "$f" 2>/dev/null | grep -q "size:exclude file"; then
        printf "%-70s  %-7s  %-7s\n" "$f" "—" "EXCLUDED"
        continue
    fi

    lines=$(effective_lines "$f" 2>/dev/null)
    lines="${lines:-0}"

    if [ "$lines" -gt "$MAX_LINES" ]; then
        printf "%-70s  %-7s  %-7s\n" "$f" "$lines" "FAIL"
        FAILED=1
        OVERSIZED_FILES+=("$f ($lines lines)")
    else
        printf "%-70s  %-7s  %-7s\n" "$f" "$lines" "PASS"
    fi
done

echo
if [ "$FAILED" -eq 0 ]; then
    echo "size-check: all touched code files are within ${MAX_LINES} lines."
    exit 0
fi

cat <<EOF
size-check: at least one touched code file exceeds ${MAX_LINES} lines.

⚠️  Refactor with SOLID principles.

Long files are a Single-Responsibility smell — when a file outgrows
this limit it almost always handles multiple concerns that should
live in their own modules. Pick the seams:

  - Are there logically distinct types/functions that don't share
    private state? Move them to siblings.
  - Is the file mixing IO with pure logic? Split.
  - Is it a god-object struct with ten unrelated methods? Trait it.

Files over the limit:
EOF
for line in "${OVERSIZED_FILES[@]}"; do
    echo "  - $line"
done

cat <<EOF

If the file is truly atomic (a long but cohesive parser state machine,
a generated table), add '// size:exclude file' on line 1 and document
the exception under "Closed items" in the PR description so the
sweep epics (20a/30a) can re-evaluate.
EOF

if [ "$MODE" = "report" ]; then
    echo
    echo "  (MODE=report — exiting 0)"
    exit 0
fi

exit 1
