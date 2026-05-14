// Pure helper: count executable fenced blocks in a markdown string.
// Recognises ```http and ```db-* (post-redesign HTTP/DB block
// formats). Other code fences (```js, ```ts, ```python, …) and the
// new placeholder kinds (mongodb / ws / graphql / sh with
// `executable=false`) are NOT counted — only blocks that ship with
// a working executor today.

const FENCE_RE = /^```(\S+)/;

const EXECUTABLE_FENCES = ["http", "db", "db-"] as const;

function isExecutableFence(token: string): boolean {
  for (const exe of EXECUTABLE_FENCES) {
    if (token === exe || token.startsWith(`${exe}-`) || token === exe.replace("-", "")) {
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
        // Closing fence (``` alone or with trailing chars treated
        // as content). Either way, leave fence state.
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
