// Epic 48 Story 03 — commit diff inspector.
//
// Pure presentational. Receives the unified-diff text already fetched
// by `gitDiff(vault, commitSha)` and renders it line-by-line in a
// monospace block. Lines are colored:
//   - additions (`+`) → accent
//   - removals  (`-`) → error
//   - headers (`@@`, `diff --git`, `index`, `+++`, `---`) → fg.3
//   - everything else → fg
//
// A CM6-based viewer with proper diff syntax highlighting can land
// later (consumer-side mount, Epic 30a sweep). For Story 03 we keep
// it simple — no editor instance to mount, no language pack to load.

import { Box, Text } from "@chakra-ui/react";

export interface GitCommitDiffViewerProps {
  /** Commit short_sha label, used only for the header. */
  shortSha?: string | null;
  /** Subject line, used for the header. */
  subject?: string | null;
  /** Raw unified-diff text. `null` while loading. */
  diff: string | null;
  /** Maximum lines to render before truncating. Default 2000. */
  maxLines?: number;
}

export function GitCommitDiffViewer({
  shortSha,
  subject,
  diff,
  maxLines = 2000,
}: GitCommitDiffViewerProps) {
  if (diff === null) {
    return (
      <Box data-testid="git-commit-diff-viewer" data-loading="true" px={3} py={4}>
        <Text fontSize="11px" color="fg.subtle">
          Loading diff…
        </Text>
      </Box>
    );
  }

  const allLines = diff.length === 0 ? [] : diff.split("\n");
  const truncated = allLines.length > maxLines;
  const lines = truncated ? allLines.slice(0, maxLines) : allLines;

  return (
    <Box
      data-testid="git-commit-diff-viewer"
      data-truncated={truncated || undefined}
      data-line-count={allLines.length}
    >
      <Box
        data-testid="git-commit-diff-viewer-header"
        px={3}
        py={2}
        borderBottomWidth="1px"
        borderBottomColor="border"
        bg="bg.subtle"
      >
        <Text fontFamily="mono" fontSize="11px" color="fg" truncate>
          {shortSha ? `${shortSha} — ` : ""}
          {subject ?? ""}
        </Text>
      </Box>
      {allLines.length === 0 ? (
        <Text
          data-testid="git-commit-diff-viewer-empty"
          fontSize="11px"
          color="fg.subtle"
          px={3}
          py={4}
        >
          No diff for this commit (likely an empty / merge commit).
        </Text>
      ) : (
        <Box as="pre" m={0} px={3} py={2} overflow="auto">
          {lines.map((line, i) => (
            <DiffLine key={i} line={line} index={i} />
          ))}
          {truncated && (
            <Text
              data-testid="git-commit-diff-viewer-truncation-hint"
              as="div"
              fontSize="10px"
              color="warn"
              mt={2}
            >
              … truncated at {maxLines} lines (full size{" "}
              {allLines.length} lines).
            </Text>
          )}
        </Box>
      )}
    </Box>
  );
}

function DiffLine({ line, index }: { line: string; index: number }) {
  const role = classifyDiffLine(line);
  return (
    <Text
      as="div"
      data-testid={`git-commit-diff-line-${index}`}
      data-role={role}
      fontFamily="mono"
      fontSize="11px"
      color={roleColor(role)}
      whiteSpace="pre"
    >
      {line || " "}
    </Text>
  );
}

export function classifyDiffLine(line: string): string {
  if (line.startsWith("@@")) return "hunk";
  if (line.startsWith("+++") || line.startsWith("---")) return "fileheader";
  if (line.startsWith("diff --git")) return "fileheader";
  if (line.startsWith("index ")) return "fileheader";
  if (line.startsWith("+")) return "add";
  if (line.startsWith("-")) return "remove";
  return "context";
}

function roleColor(role: string): string {
  switch (role) {
    case "add":
      return "brand.fg";
    case "remove":
      return "error";
    case "hunk":
      return "fg.muted";
    case "fileheader":
      return "fg.subtle";
    default:
      return "fg";
  }
}
