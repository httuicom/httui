// Names of the active environment's secret (keychain-backed) variables.
// Kept as module-level non-reactive state — the `{{ref}}` highlight reads
// it synchronously per decoration pass, the same pattern the editor uses
// for content/unsaved sets. The environment store repopulates it whenever
// the active environment or its variables change.
let secretEnvKeys: ReadonlySet<string> = new Set();

export function setSecretEnvKeys(names: Iterable<string>): void {
  secretEnvKeys = new Set(names);
}

export function isSecretEnvKey(name: string): boolean {
  return secretEnvKeys.has(name);
}
