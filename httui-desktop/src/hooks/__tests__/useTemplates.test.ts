import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useTemplates } from "@/hooks/useTemplates";
import type { Template } from "@/lib/tauri/templates";

const PG: Template = {
  id: "pg-health",
  name: "Postgres health",
  description: "heartbeat",
  source: "vault",
  body: "---\ntitle: Postgres health\n---\n```http\nGET /\n```\n",
};

beforeEach(() => clearTauriMocks());
afterEach(() => clearTauriMocks());

describe("useTemplates", () => {
  it("idles when vaultPath is null", async () => {
    const { result } = renderHook(() => useTemplates(null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.templates).toEqual([]);
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("populates the list after the initial fetch", async () => {
    mockTauriCommand("list_templates_cmd", () => [PG]);
    const { result } = renderHook(() => useTemplates("/v"));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(result.current.templates).toEqual([PG]);
  });

  it("loaded=true with templates=[] is the empty-vault signal", async () => {
    mockTauriCommand("list_templates_cmd", () => []);
    const { result } = renderHook(() => useTemplates("/v"));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(result.current.templates).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it("captures the IPC error on failure", async () => {
    mockTauriCommand("list_templates_cmd", () => {
      throw new Error("fs busy");
    });
    const { result } = renderHook(() => useTemplates("/v"));
    await waitFor(() => expect(result.current.error).toBe("fs busy"));
    expect(result.current.templates).toEqual([]);
    expect(result.current.loaded).toBe(false);
  });

  it("stringifies non-Error throws", async () => {
    mockTauriCommand("list_templates_cmd", () => {
      throw "kaboom";
    });
    const { result } = renderHook(() => useTemplates("/v"));
    await waitFor(() => expect(result.current.error).toBe("kaboom"));
  });

  it("refresh re-fetches and replaces stale data", async () => {
    let snapshot: Template[] = [PG];
    mockTauriCommand("list_templates_cmd", () => snapshot);
    const { result } = renderHook(() => useTemplates("/v"));
    await waitFor(() => expect(result.current.templates).toEqual([PG]));

    snapshot = [];
    await act(async () => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.templates).toEqual([]));
    expect(result.current.loaded).toBe(true);
  });

  it("clears state when vaultPath transitions to null", async () => {
    mockTauriCommand("list_templates_cmd", () => [PG]);
    const { result, rerender } = renderHook(
      ({ vault }: { vault: string | null }) => useTemplates(vault),
      { initialProps: { vault: "/v" as string | null } },
    );
    await waitFor(() => expect(result.current.templates).toEqual([PG]));

    rerender({ vault: null });
    await waitFor(() => expect(result.current.templates).toEqual([]));
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("error clears on successful refresh", async () => {
    let mode: "throw" | "ok" = "throw";
    mockTauriCommand("list_templates_cmd", () => {
      if (mode === "throw") throw new Error("transient");
      return [PG];
    });
    const { result } = renderHook(() => useTemplates("/v"));
    await waitFor(() => expect(result.current.error).toBe("transient"));

    mode = "ok";
    await act(async () => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toBeNull());
    expect(result.current.templates).toEqual([PG]);
  });
});
