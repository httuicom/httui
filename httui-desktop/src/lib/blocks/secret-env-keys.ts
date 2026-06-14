// Names of the active environment's secret (keychain-backed) variables.
// Kept as module-level non-reactive state — the `{{ref}}` highlight reads
// it synchronously per decoration pass, the same pattern the editor uses
// for content/unsaved sets. The environment store repopulates it whenever
// the active environment or its variables change.
let secretEnvKeys: ReadonlySet<string> = new Set();

// The set is populated asynchronously (after an IPC round-trip), often
// AFTER the editor has already painted its decorations. CM6 only rebuilds
// decorations on a transaction, so the highlight extension subscribes here
// and forces a rebuild when the set lands — otherwise a freshly-opened note
// would never flag its secret refs until the next edit.
const listeners = new Set<() => void>();

export function setSecretEnvKeys(names: Iterable<string>): void {
  secretEnvKeys = new Set(names);
  for (const listener of listeners) listener();
}

export function isSecretEnvKey(name: string): boolean {
  return secretEnvKeys.has(name);
}

/** Subscribe to set changes. Returns an unsubscribe function. */
export function subscribeSecretEnvKeys(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
