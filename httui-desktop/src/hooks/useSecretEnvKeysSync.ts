import { useEffect } from "react";
import { useCrossEnvVariables } from "@/hooks/useCrossEnvVariables";
import { useEnvironmentStore } from "@/stores/environment";
import { setSecretEnvKeys } from "@/lib/blocks/secret-env-keys";

/**
 * Keeps the module-level secret-env-key set (read by the `{{ref}}`
 * highlight) in sync with the active environment's keychain-backed
 * variables. Uses the same reactive cross-env loader the Variables page
 * uses, so it tracks vault file changes — unlike the store's one-shot
 * startup refresh, which can run before the vault is ready.
 */
export function useSecretEnvKeysSync(): void {
  const bundles = useCrossEnvVariables();
  const active = useEnvironmentStore((s) => s.activeEnvironment);

  useEffect(() => {
    const bundle = active
      ? bundles.find((b) => b.env.id === active.id)
      : undefined;
    setSecretEnvKeys(
      (bundle?.vars ?? []).filter((v) => v.is_secret).map((v) => v.key),
    );
  }, [bundles, active]);
}
