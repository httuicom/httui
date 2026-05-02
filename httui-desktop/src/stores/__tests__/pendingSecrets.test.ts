import { beforeEach, describe, expect, it } from "vitest";

import { usePendingSecretsStore } from "@/stores/pendingSecrets";
import type { MissingRef } from "@/lib/tauri/commands";

const REF_A: MissingRef = {
  source_file: "connections.toml",
  label: "postgres-prod",
  keychain_key: "conn:abc:password",
  kind: "connection",
};

const REF_B: MissingRef = {
  source_file: "envs/local.toml",
  label: "STRIPE_KEY",
  keychain_key: "env:local:STRIPE_KEY",
  kind: "env",
};

beforeEach(() => {
  usePendingSecretsStore.getState().reset();
});

describe("pendingSecrets store", () => {
  it("starts empty and closed", () => {
    const s = usePendingSecretsStore.getState();
    expect(s.pending).toEqual([]);
    expect(s.modalOpen).toBe(false);
  });

  it("setPending opens the modal when the list is non-empty", () => {
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    const s = usePendingSecretsStore.getState();
    expect(s.pending).toHaveLength(2);
    expect(s.modalOpen).toBe(true);
  });

  it("setPending leaves the modal closed when called with []", () => {
    usePendingSecretsStore.getState().setPending([]);
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
  });

  it("removePending drops the matching keychain_key", () => {
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    usePendingSecretsStore.getState().removePending(REF_A.keychain_key);
    const s = usePendingSecretsStore.getState();
    expect(s.pending).toEqual([REF_B]);
    expect(s.modalOpen).toBe(true);
  });

  it("removePending closes the modal when the list empties out", () => {
    usePendingSecretsStore.getState().setPending([REF_A]);
    usePendingSecretsStore.getState().removePending(REF_A.keychain_key);
    const s = usePendingSecretsStore.getState();
    expect(s.pending).toEqual([]);
    expect(s.modalOpen).toBe(false);
  });

  it("dismiss hides the modal but keeps refs", () => {
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    usePendingSecretsStore.getState().dismiss();
    const s = usePendingSecretsStore.getState();
    expect(s.modalOpen).toBe(false);
    expect(s.pending).toHaveLength(2);
  });

  it("reopen re-shows the modal when refs are still pending", () => {
    usePendingSecretsStore.getState().setPending([REF_A]);
    usePendingSecretsStore.getState().dismiss();
    usePendingSecretsStore.getState().reopen();
    expect(usePendingSecretsStore.getState().modalOpen).toBe(true);
  });

  it("reopen is a no-op when nothing is pending", () => {
    usePendingSecretsStore.getState().reopen();
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
  });

  it("reset wipes everything", () => {
    usePendingSecretsStore.getState().setPending([REF_A]);
    usePendingSecretsStore.getState().reset();
    const s = usePendingSecretsStore.getState();
    expect(s.pending).toEqual([]);
    expect(s.modalOpen).toBe(false);
  });
});
