#!/usr/bin/env bash
# scripts/coverage-check.sh
#
# Touched-files coverage gate. For each .rs/.ts/.tsx file changed in
# the configured diff range, require ≥80% line coverage on that file
# as a whole.
#
# Usage:
#   scripts/coverage-check.sh                # diff: HEAD~1..HEAD
#   BASE_REF=origin/main scripts/coverage-check.sh   # diff: origin/main...HEAD
#   MODE=report scripts/coverage-check.sh    # never exit non-zero
#   MODE=detect scripts/coverage-check.sh    # print has_rs/has_fe/needs_webkit
#                                            # key=value lines and exit, so CI
#                                            # can skip toolchain setup it
#                                            # won't use
#
# Files with `// coverage:exclude file` on line 1 are reported as
# EXCLUDED and don't count against the gate.
#
# Workspace-level exclusion: `httui-web/` (the marketing landing) is
# skipped entirely. It has no test setup and is purely presentational
# JSX — gating it would force coverage:exclude opt-outs on every new
# file. Add app-grade workspaces here if you spin up more landings.
#
# Exits 0 when every touched file meets the threshold (or there are
# no touched code files). Exits 1 otherwise (unless MODE=report).

set -euo pipefail

# Force C locale so awk's %f always emits "80.0", never "80,0"
# (some Brazilian/European locales use comma as decimal separator).
export LC_ALL=C
export LC_NUMERIC=C

THRESHOLD="${THRESHOLD:-80}"
MODE="${MODE:-enforce}"
BASE_REF="${BASE_REF:-}"

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

# detect_emit <has_rs> <has_fe> <needs_webkit>
# Output is GITHUB_OUTPUT-shaped so CI can pipe it straight into step
# outputs and gate its setup steps on them.
detect_emit() {
    echo "has_rs=$1"
    echo "has_fe=$2"
    echo "needs_webkit=$3"
    exit 0
}

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
        [ "$MODE" = "detect" ] && detect_emit 0 0 0
        echo "coverage-check: only one commit in branch; nothing to check"
        exit 0
    fi
    DIFF_CMD=(git diff --name-only "$DIFF_RANGE")
fi

CHANGED_FILES=()
while IFS= read -r line; do
    [ -n "$line" ] && CHANGED_FILES+=("$line")
done < <("${DIFF_CMD[@]}" 2>/dev/null \
    | grep -E '\.(rs|ts|tsx)$' \
    | grep -v -E '^httui-web/' \
    | grep -v -E '/(__tests__|tests)/' \
    | grep -v -E '(^|/)(tests|.*_tests?)\.rs$' \
    | grep -v -E '\.(test|spec|browser\.test|browser\.spec)\.(ts|tsx)$' \
    | grep -v -E '/test/' \
    | grep -v -E '\.d\.ts$' \
    | grep -v -E '(^|/)(vite|vitest|playwright|tsup|tsconfig)\.config\.(ts|tsx)$' \
    || true)

if [ ${#CHANGED_FILES[@]} -eq 0 ]; then
    [ "$MODE" = "detect" ] && detect_emit 0 0 0
    echo "coverage-check: no .rs/.ts/.tsx changes; gate skipped"
    exit 0
fi

# Filter out files that have been deleted in the working copy
KEPT=()
for f in "${CHANGED_FILES[@]}"; do
    [ -f "$f" ] && KEPT+=("$f")
done

# Check emptiness BEFORE copying: on bash 3.2 (macOS) `"${KEPT[@]}"`
# of an empty array under `set -u` is an "unbound variable" error,
# which crashed deletion-only commits before reaching this skip.
if [ ${#KEPT[@]} -eq 0 ]; then
    [ "$MODE" = "detect" ] && detect_emit 0 0 0
    echo "coverage-check: all touched files were deleted; gate skipped"
    exit 0
fi

CHANGED_FILES=("${KEPT[@]}")

HAS_RS=0
HAS_FE=0
for f in "${CHANGED_FILES[@]}"; do
    case "$f" in
        *.rs) HAS_RS=1 ;;
        *.ts | *.tsx) HAS_FE=1 ;;
    esac
done

if [ "$MODE" = "detect" ]; then
    # webkit2gtk is only required when cargo-llvm-cov will compile the
    # Tauri crate: a touched .rs under httui-desktop/src-tauri/, or an
    # unrecognized crate (which falls back to a workspace-wide run).
    NEEDS_WEBKIT=0
    for f in "${CHANGED_FILES[@]}"; do
        case "$f" in
            *.rs)
                case "$f" in
                    httui-core/* | httui-tui/* | httui-mcp/* | httui-launcher/*) ;;
                    *) NEEDS_WEBKIT=1 ;;
                esac
                ;;
        esac
    done
    detect_emit "$HAS_RS" "$HAS_FE" "$NEEDS_WEBKIT"
fi

# ---------- run coverage tools ------------------------------------------------

mkdir -p target/coverage

RUST_LCOV=target/coverage/rust.lcov
FE_LCOV=httui-desktop/coverage/lcov.info

if [ "$HAS_RS" -eq 1 ]; then
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
        echo "coverage-check: cargo-llvm-cov is not installed."
        echo "  rustup component add llvm-tools-preview"
        echo "  cargo install cargo-llvm-cov"
        exit 2
    fi
    # Detect which crates the touched .rs files belong to. Running
    # llvm-cov per-crate avoids OOM on macOS — instrumented workspace
    # builds spike memory above what 16 GB machines can handle.
    PACKAGES=()
    for f in "${CHANGED_FILES[@]}"; do
        case "$f" in
            httui-core/*) PACKAGES+=("httui-core") ;;
            httui-desktop/src-tauri/*) PACKAGES+=("httui-notes") ;;
            httui-tui/*) PACKAGES+=("httui-tui") ;;
            httui-mcp/*) PACKAGES+=("httui-mcp") ;;
            httui-launcher/*) PACKAGES+=("httui-launcher") ;;
        esac
    done
    # Dedupe
    UNIQUE_PKGS=()
    for p in "${PACKAGES[@]}"; do
        already=0
        for u in "${UNIQUE_PKGS[@]:-}"; do
            [ "$p" = "$u" ] && already=1 && break
        done
        [ "$already" -eq 0 ] && UNIQUE_PKGS+=("$p")
    done

    if [ ${#UNIQUE_PKGS[@]} -eq 0 ]; then
        echo "coverage-check: no recognized crate among touched .rs files; running workspace"
        cargo llvm-cov --workspace --lcov --output-path "$RUST_LCOV" >/dev/null
    else
        echo "coverage-check: running cargo llvm-cov for ${UNIQUE_PKGS[*]} ..."
        # Build a list of -p args
        PKG_ARGS=()
        for p in "${UNIQUE_PKGS[@]}"; do
            PKG_ARGS+=("--package" "$p")
        done
        cargo llvm-cov "${PKG_ARGS[@]}" --lcov --output-path "$RUST_LCOV" >/dev/null
    fi
fi

if [ "$HAS_FE" -eq 1 ]; then
    echo "coverage-check: running vitest --coverage ..."
    # Scope the run to tests whose import graph reaches the diff
    # (--changed): any test that can execute a touched file imports it
    # transitively, so per-file numbers match the full run. In staged
    # mode HEAD covers staged + worktree edits — a superset, fine for
    # the report-only hook. Global thresholds are meaningless over a
    # subset — zero them; the per-file check below is the enforcement.
    FE_CHANGED_REF="$BASE_REF"
    if [ "$DIFF_MODE" = "staged" ]; then
        FE_CHANGED_REF="HEAD"
    fi
    FE_ARGS=()
    if [ -n "$FE_CHANGED_REF" ]; then
        FE_ARGS+=(--changed "$FE_CHANGED_REF" --passWithNoTests
            --coverage.thresholds.lines=0
            --coverage.thresholds.functions=0
            --coverage.thresholds.statements=0
            --coverage.thresholds.branches=0)
    fi
    (cd httui-desktop && npm run --silent test -- --project unit --coverage \
        --coverage.reporter=lcov --coverage.reporter=text-summary \
        ${FE_ARGS[@]+"${FE_ARGS[@]}"} >/dev/null)
fi

# ---------- per-file lcov parser ---------------------------------------------

# extract_coverage <lcov_file> <repo_relative_path>
# Prints the line coverage percentage with 1 decimal, or "N/A" if the
# file is not present in the report.
extract_coverage() {
    local lcov="$1" target="$2"
    local abs
    abs="$(cd "$REPO_ROOT" && readlink -f "$target" 2>/dev/null || echo "$target")"

    awk -v t="$target" -v abs="$abs" '
        /^SF:/ {
            sf = substr($0, 4)
            # Match in either direction: sf may be a tail-fragment of t
            # (vitest writes httui-desktop-relative SF lines while we
            # diff repo-relative paths) or t may be a suffix of sf
            # (cargo-llvm-cov writes absolute paths in CI).
            match_now = (sf == t || sf == abs \
                || sf ~ "(^|/)" t "$" \
                || t ~ "(^|/)" sf "$")
            lf = 0; lh = 0
        }
        /^LF:/ { if (match_now) lf = substr($0, 4) }
        /^LH:/ { if (match_now) lh = substr($0, 4) }
        /^end_of_record/ {
            if (match_now && lf > 0) {
                printf("%.1f\n", (lh / lf) * 100)
                exit
            }
            match_now = 0
        }
    ' "$lcov"
}

# ---------- evaluate ---------------------------------------------------------

printf "\n%-70s  %-9s  %-7s\n" "FILE" "COVERAGE" "STATUS"
printf "%-70s  %-9s  %-7s\n" "----" "--------" "------"

# is_structural_only <file>
# True when the file has no executable code worth covering. Typical
# matches: Rust `mod.rs` that does only `pub mod`/`pub use`/derive
# enums/structs (no `fn` bodies); TS `index.ts` barrels that only
# `export ... from "..."`. Coverage tools rarely emit useful data
# for these and forcing every consumer to add an exclude comment is
# friction.
is_structural_only() {
    local f="$1"
    case "$f" in
        *.rs)
            if grep -q -E '^\s*(pub\s+)?(async\s+)?(unsafe\s+)?fn\s+\w' "$f" 2>/dev/null; then
                return 1
            fi
            if grep -q -E '^\s*impl\s+' "$f" 2>/dev/null; then
                return 1
            fi
            return 0
            ;;
        *.ts | *.tsx)
            # Barrel files: every non-blank, non-comment line is an
            # `export ... from "..."` (value or type). Anything else —
            # `const`, `function`, arrow fn, `class`, JSX — disqualifies.
            local non_export
            non_export="$(grep -v -E '^\s*(//|/\*|\*|$)' "$f" 2>/dev/null \
                | grep -v -E '^\s*export\s+(\*|\{[^}]*\}|type\s+\{[^}]*\})\s+from\s+["'"'"'][^"'"'"']+["'"'"']\s*;?\s*$' \
                || true)"
            [ -z "$non_export" ]
            ;;
        *)
            return 1
            ;;
    esac
}

FAILED=0
for f in "${CHANGED_FILES[@]}"; do
    # Escape hatch
    # Look in the first 10 lines so consumers can stack opt-outs
    # (e.g. size:exclude on line 1 + coverage:exclude on line 5).
    if head -n 10 "$f" 2>/dev/null | grep -q "coverage:exclude file"; then
        printf "%-70s  %-9s  %-7s\n" "$f" "—" "EXCLUDED"
        continue
    fi

    case "$f" in
        *.rs) lcov="$RUST_LCOV" ;;
        *.ts | *.tsx) lcov="$FE_LCOV" ;;
        *) continue ;;
    esac

    if [ ! -f "$lcov" ]; then
        printf "%-70s  %-9s  %-7s\n" "$f" "?" "NO REPORT"
        FAILED=1
        continue
    fi

    pct="$(extract_coverage "$lcov" "$f" || echo "N/A")"
    if [ -z "$pct" ] || [ "$pct" = "N/A" ]; then
        # Structural-only files (Rust `mod.rs` with re-exports + TS
        # `index.ts` barrels) have no executable lines; auto-pass.
        if is_structural_only "$f"; then
            printf "%-70s  %-9s  %-7s\n" "$f" "—" "STRUCT"
            continue
        fi
        printf "%-70s  %-9s  %-7s\n" "$f" "N/A" "MISSING"
        FAILED=1
        continue
    fi

    # Compare via awk to avoid bash's lack of float math.
    if awk -v p="$pct" -v t="$THRESHOLD" 'BEGIN { exit !(p+0 >= t+0) }'; then
        printf "%-70s  %-9s  %-7s\n" "$f" "$pct%" "PASS"
    else
        printf "%-70s  %-9s  %-7s\n" "$f" "$pct%" "FAIL"
        FAILED=1
    fi
done

echo
if [ "$FAILED" -eq 0 ]; then
    echo "coverage-check: all touched files meet ${THRESHOLD}% line coverage."
    exit 0
fi

echo "coverage-check: at least one touched file is below ${THRESHOLD}% line coverage."
echo "  - Add tests until the file reaches the threshold, OR"
echo "  - Add '// coverage:exclude file' on line 1 (use sparingly; document in tech-debt.md)"

if [ "$MODE" = "report" ]; then
    echo "  (MODE=report — exiting 0)"
    exit 0
fi

exit 1
