import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { PendingSecretsModal } from "@/components/layout/PendingSecretsModal";
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
  clearTauriMocks();
});

afterEach(() => {
  clearTauriMocks();
});

describe("PendingSecretsModal", () => {
  it("renders nothing when modalOpen is false", () => {
    renderWithProviders(<PendingSecretsModal />);
    expect(screen.queryByTestId("pending-secrets-modal")).toBeNull();
  });

  it("renders one row per pending ref when open", () => {
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    renderWithProviders(<PendingSecretsModal />);
    expect(screen.getByTestId("pending-secrets-modal")).toBeInTheDocument();
    expect(
      screen.getByTestId(`pending-secret-row-${REF_A.keychain_key}`),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId(`pending-secret-row-${REF_B.keychain_key}`),
    ).toBeInTheDocument();
  });

  it("Save calls save_secret_cmd and removes the row", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    type SaveArgs = { keychainKey: string; value: string };
    const savedRef: { current: SaveArgs | null } = { current: null };
    mockTauriCommand("save_secret_cmd", (args) => {
      savedRef.current = args as SaveArgs;
      return null;
    });

    renderWithProviders(<PendingSecretsModal />);

    const rowA = screen.getByTestId(
      `pending-secret-row-${REF_A.keychain_key}`,
    );
    const input = rowA.querySelector(
      '[data-testid="pending-secret-input"]',
    ) as HTMLInputElement;
    const saveBtn = rowA.querySelector(
      '[data-testid="pending-secret-save"]',
    ) as HTMLButtonElement;
    await user.type(input, "topsecret");
    await user.click(saveBtn);

    await waitFor(() =>
      expect(
        screen.queryByTestId(`pending-secret-row-${REF_A.keychain_key}`),
      ).toBeNull(),
    );
    expect(savedRef.current).toEqual({
      keychainKey: REF_A.keychain_key,
      value: "topsecret",
    });
    // The other row stays.
    expect(
      screen.getByTestId(`pending-secret-row-${REF_B.keychain_key}`),
    ).toBeInTheDocument();
  });

  it("Save with empty value surfaces inline error and does NOT call backend", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A]);
    const calls = vi.fn();
    mockTauriCommand("save_secret_cmd", () => {
      calls();
      return null;
    });

    renderWithProviders(<PendingSecretsModal />);
    const saveBtn = screen.getByTestId("pending-secret-save");
    await user.click(saveBtn);

    expect(screen.getByTestId("pending-secret-error").textContent).toContain(
      "valor",
    );
    expect(calls).not.toHaveBeenCalled();
    // Row still there.
    expect(
      screen.getByTestId(`pending-secret-row-${REF_A.keychain_key}`),
    ).toBeInTheDocument();
  });

  it("Save shows backend error inline without removing the row", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A]);
    mockTauriCommand("save_secret_cmd", () => {
      throw new Error("keychain locked");
    });

    renderWithProviders(<PendingSecretsModal />);
    await user.type(
      screen.getByTestId("pending-secret-input"),
      "topsecret",
    );
    await user.click(screen.getByTestId("pending-secret-save"));

    await waitFor(() =>
      expect(
        screen.getByTestId("pending-secret-error").textContent,
      ).toContain("keychain locked"),
    );
    // Row stayed because save failed.
    expect(
      screen.getByTestId(`pending-secret-row-${REF_A.keychain_key}`),
    ).toBeInTheDocument();
  });

  it("Skip per-row hides the row from the visible list but keeps the ref pending in the store", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    renderWithProviders(<PendingSecretsModal />);

    const rowA = screen.getByTestId(
      `pending-secret-row-${REF_A.keychain_key}`,
    );
    const skipBtn = rowA.querySelector(
      '[data-testid="pending-secret-skip"]',
    ) as HTMLButtonElement;
    await user.click(skipBtn);

    // Row A no longer rendered — but still in the global store so
    // the StatusBar badge keeps counting it.
    expect(
      screen.queryByTestId(`pending-secret-row-${REF_A.keychain_key}`),
    ).toBeNull();
    expect(usePendingSecretsStore.getState().pending).toHaveLength(2);
    // Row B still visible.
    expect(
      screen.getByTestId(`pending-secret-row-${REF_B.keychain_key}`),
    ).toBeInTheDocument();
  });

  it("auto-closes the modal when every row has been Skipped (refs stay pending in store)", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);
    renderWithProviders(<PendingSecretsModal />);

    // Skip both rows.
    const rowA = screen.getByTestId(
      `pending-secret-row-${REF_A.keychain_key}`,
    );
    await user.click(
      rowA.querySelector(
        '[data-testid="pending-secret-skip"]',
      ) as HTMLButtonElement,
    );
    const rowB = screen.getByTestId(
      `pending-secret-row-${REF_B.keychain_key}`,
    );
    await user.click(
      rowB.querySelector(
        '[data-testid="pending-secret-skip"]',
      ) as HTMLButtonElement,
    );

    // Modal closed itself — no empty header/footer surface left
    // hovering on screen.
    await waitFor(() =>
      expect(screen.queryByTestId("pending-secrets-modal")).toBeNull(),
    );
    // But both refs still pending in the store, so the badge keeps
    // counting them.
    expect(usePendingSecretsStore.getState().pending).toHaveLength(2);
  });

  it("Skip per-row never calls save_secret_cmd", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A]);
    const saveCalls = vi.fn();
    mockTauriCommand("save_secret_cmd", () => {
      saveCalls();
      return null;
    });

    renderWithProviders(<PendingSecretsModal />);
    await user.click(screen.getByTestId("pending-secret-skip"));

    expect(saveCalls).not.toHaveBeenCalled();
  });

  it("re-opening the modal after Skip-per-row + dismiss shows the skipped row again", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A]);
    renderWithProviders(<PendingSecretsModal />);

    // Skip then dismiss.
    await user.click(screen.getByTestId("pending-secret-skip"));
    expect(
      screen.queryByTestId(`pending-secret-row-${REF_A.keychain_key}`),
    ).toBeNull();
    await act(async () => {
      usePendingSecretsStore.getState().dismiss();
    });

    // Reopen via the store. The session-local "skipped" set should
    // have been cleared by the modal's open-effect, so REF_A shows
    // up again.
    await act(async () => {
      usePendingSecretsStore.getState().reopen();
    });
    await waitFor(() =>
      expect(
        screen.getByTestId(`pending-secret-row-${REF_A.keychain_key}`),
      ).toBeInTheDocument(),
    );
  });

  it("Skip all dismisses the modal but keeps refs in the store", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A, REF_B]);

    renderWithProviders(<PendingSecretsModal />);
    await user.click(screen.getByTestId("pending-secrets-skip-all"));

    expect(screen.queryByTestId("pending-secrets-modal")).toBeNull();
    expect(usePendingSecretsStore.getState().pending).toHaveLength(2);
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
  });

  it("Done dismisses the modal", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A]);
    renderWithProviders(<PendingSecretsModal />);
    await user.click(screen.getByTestId("pending-secrets-done"));
    expect(screen.queryByTestId("pending-secrets-modal")).toBeNull();
  });

  it("modal closes automatically when the last row is saved", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([REF_A]);
    mockTauriCommand("save_secret_cmd", () => null);

    renderWithProviders(<PendingSecretsModal />);
    await user.type(
      screen.getByTestId("pending-secret-input"),
      "topsecret",
    );
    await user.click(screen.getByTestId("pending-secret-save"));

    await waitFor(() =>
      expect(screen.queryByTestId("pending-secrets-modal")).toBeNull(),
    );
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
  });
});
