import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { defaultSuggestionProvider } from "@/components/blocks/preflight/preflight-suggestions";
import { useEnvironmentStore } from "@/stores/environment";
import {
  clearTauriMocks,
  mockTauriCommand,
} from "@/test/mocks/tauri";

describe("defaultSuggestionProvider", () => {
  beforeEach(() => {
    clearTauriMocks();
  });

  afterEach(() => {
    clearTauriMocks();
  });

  it("connection: returns connection names from listConnections", async () => {
    mockTauriCommand("list_connections", () => [
      { id: "1", name: "payments-db", driver: "postgres" },
      { id: "2", name: "audit-db", driver: "postgres" },
      { id: "3", name: "", driver: "postgres" },
    ]);
    const out = await defaultSuggestionProvider("connection");
    expect(out).toEqual(["payments-db", "audit-db"]);
  });

  it("connection: empty array on Tauri error", async () => {
    mockTauriCommand("list_connections", () => {
      throw new Error("rpc fail");
    });
    expect(await defaultSuggestionProvider("connection")).toEqual([]);
  });

  it("env_var: returns sorted active env keys", async () => {
    useEnvironmentStore.setState({
      getActiveVariables: async () => ({
        ZULU: "z",
        ALPHA: "a",
        MIKE: "m",
      }),
    } as Partial<ReturnType<typeof useEnvironmentStore.getState>> as never);
    expect(await defaultSuggestionProvider("env_var")).toEqual([
      "ALPHA",
      "MIKE",
      "ZULU",
    ]);
  });

  it("env_var: empty array on store error", async () => {
    useEnvironmentStore.setState({
      getActiveVariables: async () => {
        throw new Error("env fail");
      },
    } as Partial<ReturnType<typeof useEnvironmentStore.getState>> as never);
    expect(await defaultSuggestionProvider("env_var")).toEqual([]);
  });

  it("text-only kinds return empty", async () => {
    expect(await defaultSuggestionProvider("branch")).toEqual([]);
    expect(await defaultSuggestionProvider("keychain")).toEqual([]);
    expect(await defaultSuggestionProvider("file_exists")).toEqual([]);
    expect(await defaultSuggestionProvider("command")).toEqual([]);
  });
});
