import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { usePendingSecretsScan } from "@/hooks/usePendingSecretsScan";
import { usePendingSecretsStore } from "@/stores/pendingSecrets";
import { useWorkspaceStore } from "@/stores/workspace";
import type { MissingRef } from "@/lib/tauri/commands";

const REF_A: MissingRef = {
  source_file: "connections.toml",
  label: "postgres-prod",
  keychain_key: "conn:abc:password",
  kind: "connection",
};

beforeEach(() => {
  useWorkspaceStore.setState({ vaultPath: null });
  usePendingSecretsStore.getState().reset();
  clearTauriMocks();
});

afterEach(() => {
  clearTauriMocks();
});

describe("usePendingSecretsScan", () => {
  it("does nothing when vaultPath is null", () => {
    let calls = 0;
    mockTauriCommand("list_missing_secrets", () => {
      calls += 1;
      return [];
    });
    renderHook(() => usePendingSecretsScan());
    expect(calls).toBe(0);
    expect(usePendingSecretsStore.getState().pending).toEqual([]);
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
  });

  it("scans on mount when vaultPath is set; pushes refs into the store", async () => {
    useWorkspaceStore.setState({ vaultPath: "/v" });
    mockTauriCommand("list_missing_secrets", () => [REF_A]);
    renderHook(() => usePendingSecretsScan());
    await waitFor(() => {
      const s = usePendingSecretsStore.getState();
      expect(s.pending).toEqual([REF_A]);
      expect(s.modalOpen).toBe(true);
    });
  });

  it("re-scans when vaultPath changes", async () => {
    let lastCallVault: string | null = null;
    mockTauriCommand("list_missing_secrets", (args) => {
      lastCallVault = (args as { vaultPath: string }).vaultPath;
      return [];
    });

    useWorkspaceStore.setState({ vaultPath: "/v1" });
    renderHook(() => usePendingSecretsScan());
    await waitFor(() => expect(lastCallVault).toBe("/v1"));

    useWorkspaceStore.setState({ vaultPath: "/v2" });
    await waitFor(() => expect(lastCallVault).toBe("/v2"));
  });

  it("resets the store when vaultPath transitions back to null", async () => {
    useWorkspaceStore.setState({ vaultPath: "/v" });
    mockTauriCommand("list_missing_secrets", () => [REF_A]);
    const { rerender } = renderHook(() => usePendingSecretsScan());
    await waitFor(() =>
      expect(usePendingSecretsStore.getState().pending).toHaveLength(1),
    );

    useWorkspaceStore.setState({ vaultPath: null });
    rerender();
    await waitFor(() =>
      expect(usePendingSecretsStore.getState().pending).toEqual([]),
    );
  });

  it("swallows backend errors without throwing or polluting the store", async () => {
    useWorkspaceStore.setState({ vaultPath: "/v" });
    mockTauriCommand("list_missing_secrets", () => {
      throw new Error("io");
    });
    expect(() => renderHook(() => usePendingSecretsScan())).not.toThrow();
    // Give the rejected promise a tick to settle.
    await new Promise((r) => setTimeout(r, 10));
    expect(usePendingSecretsStore.getState().pending).toEqual([]);
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
  });
});
