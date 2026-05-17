import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type { Environment, EnvVariable } from "@/lib/tauri/commands";
import {
  listEnvironments,
  createEnvironment as createEnvCmd,
  deleteEnvironment as deleteEnvCmd,
  duplicateEnvironment as duplicateEnvCmd,
  renameEnvironment as renameEnvCmd,
  setActiveEnvironment as setActiveEnvCmd,
  listEnvVariables,
  setEnvVariable as setEnvVarCmd,
  deleteEnvVariable as deleteEnvVarCmd,
  resolveActiveEnvVariables,
} from "@/lib/tauri/commands";
import { useSessionOverrideStore } from "./sessionOverride";

// --- Types ---

interface EnvironmentState {
  environments: Environment[];
  activeEnvironment: Environment | null;
  managerOpen: boolean;
  variablesVersion: number;

  // Actions
  openManager: () => void;
  closeManager: () => void;
  refresh: () => Promise<void>;
  switchEnvironment: (id: string | null) => Promise<void>;
  createEnvironment: (name: string) => Promise<void>;
  deleteEnvironment: (id: string) => Promise<void>;
  duplicateEnvironment: (sourceId: string, newName: string) => Promise<void>;
  renameEnvironment: (oldId: string, newName: string) => Promise<void>;
  loadVariables: (environmentId: string) => Promise<EnvVariable[]>;
  setVariable: (
    environmentId: string,
    key: string,
    value: string,
    isSecret?: boolean,
  ) => Promise<EnvVariable>;
  deleteVariable: (id: string) => Promise<void>;
  getActiveVariables: () => Promise<Record<string, string>>;
}

// --- Store ---

export const useEnvironmentStore = create<EnvironmentState>()(
  devtools(
    (set, get) => ({
      environments: [],
      activeEnvironment: null,
      managerOpen: false,
      variablesVersion: 0,

      openManager: () => set({ managerOpen: true }),
      closeManager: () => set({ managerOpen: false }),

      refresh: async () => {
        try {
          const envs = await listEnvironments();
          set({
            environments: envs,
            activeEnvironment: envs.find((e) => e.is_active) ?? null,
          });
        } catch {
          /* silently fail */
        }
      },

      switchEnvironment: async (id) => {
        await setActiveEnvCmd(id);
        await get().refresh();
      },

      createEnvironment: async (name) => {
        await createEnvCmd(name);
        await get().refresh();
      },

      deleteEnvironment: async (id) => {
        await deleteEnvCmd(id);
        await get().refresh();
      },

      duplicateEnvironment: async (sourceId, newName) => {
        await duplicateEnvCmd(sourceId, newName);
        await get().refresh();
      },

      renameEnvironment: async (oldId, newName) => {
        await renameEnvCmd(oldId, newName);
        await get().refresh();
      },

      loadVariables: async (environmentId) => {
        return listEnvVariables(environmentId);
      },

      setVariable: async (environmentId, key, value, isSecret) => {
        const result = await setEnvVarCmd(environmentId, key, value, isSecret);
        set((state) => ({ variablesVersion: state.variablesVersion + 1 }));
        return result;
      },

      deleteVariable: async (id) => {
        await deleteEnvVarCmd(id);
        set((state) => ({ variablesVersion: state.variablesVersion + 1 }));
      },

      getActiveVariables: async () => {
        const { activeEnvironment } = get();
        if (!activeEnvironment) return {};
        // The dedicated execution-context IPC resolves secret values
        // from the keychain. The plain `listEnvVariables` masks
        // secrets to empty strings — using it here would silently
        // collapse `{{SECRET_KEY}}` to nothing on every request.
        let resolved: Record<string, string>;
        try {
          resolved = await resolveActiveEnvVariables();
        } catch {
          // Fall back to the masked list so the request still goes
          // out (with secrets unresolved) rather than failing.
          const vars = await listEnvVariables(activeEnvironment.id);
          resolved = {};
          for (const v of vars) {
            resolved[v.key] = v.value;
          }
        }
        // V5 cenário 3 — apply session overrides for the active env on
        // top of the resolved values so block runs see the TEMPORARY
        // value the user set, not the vault-stored one.
        const overrides =
          useSessionOverrideStore.getState().overrides[
            activeEnvironment.name
          ] ?? {};
        return { ...resolved, ...overrides };
      },
    }),
    { name: "environment-store" },
  ),
);
