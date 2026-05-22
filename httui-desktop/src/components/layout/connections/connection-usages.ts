// Pure scanner: walks markdown text and returns the line numbers where a fenced
// code block opens with the `db-<connection_id>` info-string. File paths come
// from the consumer (one (path, content) tuple at a time).

export interface RunbookUsage {
  filePath: string;
  /** 1-based line number of the opening fence. */
  line: number;
  /** A short snippet from the line right below the fence — e.g.
   * the SQL `SELECT … FROM …` text — for hovercard preview.
   * `null` when the fence is the last non-empty line. */
  preview: string | null;
}

const FENCE_OPEN_RE = /^```/;

/** Find every opening fence whose info-string starts with `db-<connectionId>`.
 * Token boundaries are whitespace: `db-c1 alias=x` matches but `db-c10` does NOT. */
export function findUsagesInFile(
  filePath: string,
  content: string,
  connectionId: string,
): RunbookUsage[] {
  const lines = content.split(/\r?\n/);
  const target = `db-${connectionId}`;
  const out: RunbookUsage[] = [];
  let inside = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!FENCE_OPEN_RE.test(line)) continue;
    // Closing fence?
    if (inside) {
      inside = false;
      continue;
    }
    // Opening fence — examine info string.
    const info = line.slice(3).trim(); // drop the leading ```
    const firstToken = info.split(/\s+/, 1)[0] ?? "";
    if (firstToken === target) {
      const next = lines[i + 1] ?? "";
      const preview = next.trim().length > 0 ? next.trim().slice(0, 80) : null;
      out.push({ filePath, line: i + 1, preview });
    }
    inside = true;
  }
  return out;
}

/** Walk a list of (path, content) pairs and aggregate usages across the vault
 * for one connection. */
export function findUsagesAcrossVault(
  files: { path: string; content: string }[],
  connectionId: string,
): RunbookUsage[] {
  return files.flatMap((f) =>
    findUsagesInFile(f.path, f.content, connectionId),
  );
}
