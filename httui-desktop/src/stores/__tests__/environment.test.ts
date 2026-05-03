import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { useEnvironmentStore } from "@/stores/environment";
import { useSessionOverrideStore } from "@/stores/sessionOverride";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { Environment, EnvVariable } from "@/lib/tauri/commands";

const mkEnv = (id: string, name: string, isActive = false): Environment => ({
  id,
  name,
  is_active: isActive,
  created_at: "2026-01-01T00:00:00Z",
});

const mkVar = (
  id: string,
  envId: string,
  key: string,
  value: string,
  isSecret = false,
): EnvVariable => ({
  id,
  environment_id: envId,
  key,
  value,
  is_secret: isSecret,
  created_at: "2026-01-01T00:00:00Z",
});

function resetStore() {
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    managerOpen: false,
    variablesVersion: 0,
  });
}

describe("environmentStore", () => {
  beforeEach(() => {
    resetStore();
    clearTauriMocks();
  });

  afterEach(() => {
    clearTauriMocks();
  });

  describe("manager open/close", () => {
    it("openManager flips managerOpen to true", () => {
      useEnvironmentStore.getState().openManager();
      expect(useEnvironmentStore.getState().managerOpen).toBe(true);
    });

    it("closeManager flips managerOpen to false", () => {
      useEnvironmentStore.setState({ managerOpen: true });
      useEnvironmentStore.getState().closeManager();
      expect(useEnvironmentStore.getState().managerOpen).toBe(false);
    });
  });

  describe("refresh", () => {
    it("loads environments and picks the active one", async () => {
      const envs = [mkEnv("a", "dev"), mkEnv("b", "prod", true)];
      mockTauriCommand("list_environments", () => envs);

      await useEnvironmentStore.getState().refresh();

      expect(useEnvironmentStore.getState().environments).toEqual(envs);
      expect(useEnvironmentStore.getState().activeEnvironment?.id).toBe("b");
    });

    it("sets activeEnvironment to null when none is active", async () => {
      mockTauriCommand("list_environments", () => [mkEnv("a", "dev")]);

      await useEnvironmentStore.getState().refresh();

      expect(useEnvironmentStore.getState().activeEnvironment).toBeNull();
    });

    it("silently swallows errors", async () => {
      mockTauriCommand("list_environments", () => {
        throw new Error("db down");
      });

      await expect(
        useEnvironmentStore.getState().refresh(),
      ).resolves.toBeUndefined();

      expect(useEnvironmentStore.getState().environments).toEqual([]);
    });
  });

  describe("switchEnvironment", () => {
    it("calls set_active_environment then refreshes", async () => {
      let activeArg: unknown = "untouched";
      mockTauriCommand("set_active_environment", (args) => {
        activeArg = (args as { id: string | null })?.id;
      });
      mockTauriCommand("list_environments", () => [mkEnv("x", "x", true)]);

      await useEnvironmentStore.getState().switchEnvironment("x");

      expect(activeArg).toBe("x");
      expect(useEnvironmentStore.getState().activeEnvironment?.id).toBe("x");
    });

    it("accepts null id (clearing active)", async () => {
      let activeArg: unknown = "untouched";
      mockTauriCommand("set_active_environment", (args) => {
        activeArg = (args as { id: string | null })?.id;
      });
      mockTauriCommand("list_environments", () => []);

      await useEnvironmentStore.getState().switchEnvironment(null);

      expect(activeArg).toBeNull();
    });
  });

  describe("createEnvironment", () => {
    it("creates and refreshes", async () => {
      let createdName: unknown = "";
      mockTauriCommand("create_environment", (args) => {
        createdName = (args as { name: string }).name;
      });
      mockTauriCommand("list_environments", () => [mkEnv("a", "new", true)]);

      await useEnvironmentStore.getState().createEnvironment("new");

      expect(createdName).toBe("new");
      expect(useEnvironmentStore.getState().environments).toHaveLength(1);
    });
  });

  describe("deleteEnvironment", () => {
    it("deletes and refreshes", async () => {
      let deletedId: unknown = "";
      mockTauriCommand("delete_environment", (args) => {
        deletedId = (args as { id: string }).id;
      });
      mockTauriCommand("list_environments", () => []);

      await useEnvironmentStore.getState().deleteEnvironment("a");

      expect(deletedId).toBe("a");
      expect(useEnvironmentStore.getState().environments).toEqual([]);
    });
  });

  describe("duplicateEnvironment", () => {
    it("duplicates with new name and refreshes", async () => {
      let receivedArgs: unknown = null;
      mockTauriCommand("duplicate_environment", (args) => {
        receivedArgs = args;
      });
      mockTauriCommand("list_environments", () => [
        mkEnv("a", "dev"),
        mkEnv("b", "dev-copy"),
      ]);

      await useEnvironmentStore
        .getState()
        .duplicateEnvironment("a", "dev-copy");

      expect(receivedArgs).toEqual({ sourceId: "a", newName: "dev-copy" });
      expect(useEnvironmentStore.getState().environments).toHaveLength(2);
    });
  });

  describe("loadVariables", () => {
    it("returns variables for the given environment", async () => {
      const vars = [mkVar("v1", "a", "TOKEN", "abc")];
      mockTauriCommand("list_env_variables", () => vars);

      const result = await useEnvironmentStore.getState().loadVariables("a");

      expect(result).toEqual(vars);
    });
  });

  describe("setVariable", () => {
    it("invokes set_env_variable and bumps variablesVersion", async () => {
      const created = mkVar("v1", "a", "K", "V");
      let received: unknown = null;
      mockTauriCommand("set_env_variable", (args) => {
        received = args;
        return created;
      });

      const before = useEnvironmentStore.getState().variablesVersion;
      const result = await useEnvironmentStore
        .getState()
        .setVariable("a", "K", "V");

      expect(received).toEqual({
        environmentId: "a",
        key: "K",
        value: "V",
        isSecret: undefined,
      });
      expect(result).toEqual(created);
      expect(useEnvironmentStore.getState().variablesVersion).toBe(before + 1);
    });

    it("forwards isSecret flag when provided", async () => {
      let received: unknown = null;
      mockTauriCommand("set_env_variable", (args) => {
        received = args;
        return mkVar("v1", "a", "S", "shh", true);
      });

      await useEnvironmentStore.getState().setVariable("a", "S", "shh", true);

      expect((received as { isSecret: boolean }).isSecret).toBe(true);
    });
  });

  describe("deleteVariable", () => {
    it("invokes delete_env_variable and bumps variablesVersion", async () => {
      let deletedId: unknown = "";
      mockTauriCommand("delete_env_variable", (args) => {
        deletedId = (args as { id: string }).id;
      });

      const before = useEnvironmentStore.getState().variablesVersion;
      await useEnvironmentStore.getState().deleteVariable("v1");

      expect(deletedId).toBe("v1");
      expect(useEnvironmentStore.getState().variablesVersion).toBe(before + 1);
    });
  });

  describe("getActiveVariables", () => {
    it("returns empty object when no active environment", async () => {
      const result = await useEnvironmentStore.getState().getActiveVariables();
      expect(result).toEqual({});
    });

    it("returns key->value map for the active environment (resolved)", async () => {
      useEnvironmentStore.setState({
        activeEnvironment: mkEnv("a", "dev", true),
      });
      // Primary path is the resolver IPC — secrets come back already
      // unmasked. The plain `list_env_variables` is the fallback.
      mockTauriCommand("resolve_active_env_variables", () => ({
        TOKEN: "abc",
        URL: "https://example.com",
      }));

      const result = await useEnvironmentStore.getState().getActiveVariables();

      expect(result).toEqual({
        TOKEN: "abc",
        URL: "https://example.com",
      });
    });

    it("returns empty object when active env has no variables", async () => {
      useEnvironmentStore.setState({
        activeEnvironment: mkEnv("a", "dev", true),
      });
      mockTauriCommand("resolve_active_env_variables", () => ({}));

      const result = await useEnvironmentStore.getState().getActiveVariables();
      expect(result).toEqual({});
    });

    it("falls back to listEnvVariables when the resolver IPC fails", async () => {
      useEnvironmentStore.setState({
        activeEnvironment: mkEnv("a", "dev", true),
      });
      mockTauriCommand("resolve_active_env_variables", () => {
        throw new Error("backend offline");
      });
      mockTauriCommand("list_env_variables", () => [
        mkVar("v1", "a", "TOKEN", "abc"),
      ]);

      const result = await useEnvironmentStore.getState().getActiveVariables();
      expect(result).toEqual({ TOKEN: "abc" });
    });

    it("session overrides for the active env shadow the resolved values (V5)", async () => {
      useEnvironmentStore.setState({
        activeEnvironment: mkEnv("a", "dev", true),
      });
      mockTauriCommand("resolve_active_env_variables", () => ({
        TOKEN: "from-vault",
        URL: "https://example.com",
      }));
      useSessionOverrideStore
        .getState()
        .setOverride("dev", "TOKEN", "from-override");

      const result = await useEnvironmentStore.getState().getActiveVariables();

      expect(result).toEqual({
        TOKEN: "from-override",
        URL: "https://example.com",
      });
      useSessionOverrideStore.getState().clearAll();
    });
  });
});
