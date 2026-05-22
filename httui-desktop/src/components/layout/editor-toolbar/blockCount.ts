// Count executable fenced blocks (```http and ```db-*) in a markdown string.
// Non-executable fences (js, ts, python, placeholder kinds) are not counted.

const FENCE_RE = /^```(\S+)/;

const EXECUTABLE_FENCES = ["http", "db", "db-"] as const;

function isExecutableFence(token: string): boolean {
  for (const exe of EXECUTABLE_FENCES) {
    if (
      token === exe ||
      token.startsWith(`${exe}-`) ||
      token === exe.replace("-", "")
    ) {
      return true;
    }
  }
  return false;
}

/** Count the number of executable fenced blocks in `content`. Walks
 * lines once; fences open + close on `^````. Skips nested fences in
 * the count. */
export function countExecutableBlocks(content: string): number {
  let count = 0;
  let inFence = false;

  for (const raw of content.split("\n")) {
    const line = raw.trimEnd();
    const match = FENCE_RE.exec(line);
    if (match) {
      if (!inFence) {
        inFence = true;
        if (isExecutableFence(match[1].toLowerCase())) count += 1;
      } else {
        inFence = false;
      }
      continue;
    }
    // Plain ``` line closes a fence too.
    if (line === "```" && inFence) {
      inFence = false;
    }
  }

  return count;
}
