import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { EmptyVaultScreen } from "@/components/layout/EmptyVaultScreen";
import { useWorkspaceStore } from "@/stores/workspace";

// Stub the Tauri dialog plugin — the EmptyVaultScreen lazy-imports it.
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import { open as openDialog } from "@tauri-apps/plugin-dialog";

beforeEach(() => {
  useWorkspaceStore.setState({
    vaultPath: null,
    vaults: [],
    entries: [],
  });
  clearTauriMocks();
  vi.mocked(openDialog).mockReset();
});

afterEach(() => {
  clearTauriMocks();
});

describe("EmptyVaultScreen", () => {
  it("renders the welcome heading and three V1 cards", () => {
    renderWithProviders(<EmptyVaultScreen />);
    expect(
      screen.getByRole("heading", { name: /Welcome to httui notes/i }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("open-vault-card")).toBeInTheDocument();
    expect(screen.getByTestId("clone-vault-card")).toBeInTheDocument();
    expect(screen.getByTestId("create-vault-card")).toBeInTheDocument();
  });

  it("does NOT render the v1.x-only Templates and Importar cards", () => {
    renderWithProviders(<EmptyVaultScreen />);
    expect(screen.queryByTestId("templates-card")).toBeNull();
    expect(screen.queryByTestId("importar-card")).toBeNull();
    expect(screen.queryByTestId("em-branco-card")).toBeNull();
  });

  it("Open card dispatches the workspace store openVault action", async () => {
    const user = userEvent.setup();
    const spy = vi.fn();
    useWorkspaceStore.setState({ openVault: spy });

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("open-vault-card"));

    expect(spy).toHaveBeenCalledTimes(1);
  });

  it("Open card surfaces an inline error if openVault rejects", async () => {
    const user = userEvent.setup();
    const spy = vi.fn(async () => {
      throw new Error("vault not a git repo");
    });
    useWorkspaceStore.setState({ openVault: spy });

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("open-vault-card"));

    await waitFor(() =>
      expect(screen.getByTestId("empty-vault-error")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("empty-vault-error").textContent).toContain(
      "vault not a git repo",
    );
  });

  it("Clone card calls clone_vault_cmd with null parent and switches into derived path", async () => {
    const user = userEvent.setup();
    type CloneArgs = { url: string; parent: string | null };
    const cloneArgsRef: { current: CloneArgs | null } = { current: null };
    mockTauriCommand("clone_vault_cmd", (args) => {
      cloneArgsRef.current = args as CloneArgs;
      return { destination: "/Users/me/Documents/y" };
    });
    mockTauriCommand("set_active_vault", () => null);
    mockTauriCommand("list_workspace", () => []);
    mockTauriCommand("start_watching", () => null);
    mockTauriCommand("rebuild_search_index", () => null);
    mockTauriCommand("stop_watching", () => null);

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("clone-vault-expand"));
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "https://github.com/x/y.git",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));

    await waitFor(() =>
      expect(useWorkspaceStore.getState().vaultPath).toBe(
        "/Users/me/Documents/y",
      ),
    );
    expect(cloneArgsRef.current).toEqual({
      url: "https://github.com/x/y.git",
      parent: null,
    });
  });

  it("Clone card surfaces backend error inline without doubling the global banner", async () => {
    const user = userEvent.setup();
    mockTauriCommand("clone_vault_cmd", () => {
      throw new Error("fatal: repository 'foo' not found");
    });

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("clone-vault-expand"));
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "https://nope.invalid/x.git",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));

    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-error").textContent).toContain(
        "fatal",
      ),
    );
    // Inline-only — the screen-level banner stays hidden so the
    // user doesn't read the same message twice.
    expect(screen.queryByTestId("empty-vault-error")).toBeNull();
    expect(screen.getByTestId("open-vault-card")).toBeInTheDocument();
    expect(useWorkspaceStore.getState().vaultPath).toBeNull();
  });

  it("Create card calls create_vault_cmd and switches into the new vault", async () => {
    const user = userEvent.setup();
    vi.mocked(openDialog).mockResolvedValueOnce("/tmp/parent");
    type CreateArgs = { parentPath: string; name: string };
    const createArgsRef: { current: CreateArgs | null } = { current: null };
    mockTauriCommand("create_vault_cmd", (args) => {
      createArgsRef.current = args as CreateArgs;
      return {
        destination: "/tmp/parent/meu-vault",
        scaffold: {
          vault_path: "/tmp/parent/meu-vault",
          created: ["connections.toml"],
          already_a_vault: false,
        },
      };
    });
    mockTauriCommand("set_active_vault", () => null);
    mockTauriCommand("list_workspace", () => []);
    mockTauriCommand("start_watching", () => null);
    mockTauriCommand("rebuild_search_index", () => null);
    mockTauriCommand("stop_watching", () => null);

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("create-vault-expand"));
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-parent").textContent).toContain(
        "/tmp/parent",
      ),
    );
    await user.type(screen.getByTestId("create-vault-name"), "meu-vault");
    await user.click(screen.getByTestId("create-vault-submit"));

    await waitFor(() =>
      expect(useWorkspaceStore.getState().vaultPath).toBe(
        "/tmp/parent/meu-vault",
      ),
    );
    expect(createArgsRef.current).toEqual({
      parentPath: "/tmp/parent",
      name: "meu-vault",
    });
  });

  it("Create card surfaces backend error inline without doubling the global banner", async () => {
    const user = userEvent.setup();
    vi.mocked(openDialog).mockResolvedValueOnce("/tmp/parent");
    mockTauriCommand("create_vault_cmd", () => {
      throw new Error("'/tmp/parent/exists' já existe e não está vazio");
    });

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("create-vault-expand"));
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-parent").textContent).toContain(
        "/tmp/parent",
      ),
    );
    await user.type(screen.getByTestId("create-vault-name"), "exists");
    await user.click(screen.getByTestId("create-vault-submit"));

    await waitFor(() =>
      expect(screen.getByTestId("create-vault-error").textContent).toContain(
        "já existe",
      ),
    );
    expect(screen.queryByTestId("empty-vault-error")).toBeNull();
    expect(useWorkspaceStore.getState().vaultPath).toBeNull();
  });

  it("Sidebar 'Novo runbook' scaffolds + switches into the chosen folder", async () => {
    const user = userEvent.setup();
    vi.mocked(openDialog).mockResolvedValue("/tmp/sidebar-vault");
    let scaffolded: string | null = null;
    mockTauriCommand("scaffold_vault", (args) => {
      scaffolded = (args as { vaultPath: string }).vaultPath;
      return {
        vault_path: scaffolded,
        created: ["connections.toml"],
        already_a_vault: false,
      };
    });
    mockTauriCommand("set_active_vault", () => null);
    mockTauriCommand("list_workspace", () => []);
    mockTauriCommand("start_watching", () => null);
    mockTauriCommand("rebuild_search_index", () => null);
    mockTauriCommand("stop_watching", () => null);

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("create-runbook-btn"));

    await waitFor(() =>
      expect(useWorkspaceStore.getState().vaultPath).toBe("/tmp/sidebar-vault"),
    );
    expect(scaffolded).toBe("/tmp/sidebar-vault");
  });

  it("Sidebar create surfaces inline error when scaffold rejects", async () => {
    const user = userEvent.setup();
    vi.mocked(openDialog).mockResolvedValue("/tmp/bad");
    mockTauriCommand("scaffold_vault", () => {
      throw new Error("permission denied");
    });

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("create-runbook-btn"));

    await waitFor(() =>
      expect(screen.getByTestId("empty-vault-error")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("empty-vault-error").textContent).toContain(
      "permission denied",
    );
    expect(useWorkspaceStore.getState().vaultPath).toBeNull();
  });

  it("Sidebar create is a no-op when the user cancels the picker", async () => {
    const user = userEvent.setup();
    vi.mocked(openDialog).mockResolvedValue(null);
    let scaffoldCalled = false;
    mockTauriCommand("scaffold_vault", () => {
      scaffoldCalled = true;
      return null;
    });

    renderWithProviders(<EmptyVaultScreen />);
    await user.click(screen.getByTestId("create-runbook-btn"));

    await new Promise((r) => setTimeout(r, 10));
    expect(scaffoldCalled).toBe(false);
    expect(useWorkspaceStore.getState().vaultPath).toBeNull();
  });

  it("Pasting a URL scaffolds + writes the seed runbook (Story 06 carry)", async () => {
    vi.mocked(openDialog).mockResolvedValue("/tmp/paste-vault");
    type WriteArgs = { vaultPath: string; filePath: string; content: string };
    const scaffoldedRef: { current: string | null } = { current: null };
    const writtenRef: { current: WriteArgs | null } = { current: null };
    mockTauriCommand("scaffold_vault", (args) => {
      scaffoldedRef.current = (args as { vaultPath: string }).vaultPath;
      return {
        vault_path: scaffoldedRef.current,
        created: ["connections.toml"],
        already_a_vault: false,
      };
    });
    mockTauriCommand("write_note", (args) => {
      writtenRef.current = args as WriteArgs;
      return null;
    });
    mockTauriCommand("set_active_vault", () => null);
    mockTauriCommand("list_workspace", () => []);
    mockTauriCommand("start_watching", () => null);
    mockTauriCommand("rebuild_search_index", () => null);
    mockTauriCommand("stop_watching", () => null);

    renderWithProviders(<EmptyVaultScreen />);

    const paste = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(paste, "clipboardData", {
      value: {
        getData: (type: string) =>
          type === "text/plain" ? "https://api.example.com/users" : "",
      },
    });
    document.dispatchEvent(paste);

    await waitFor(() =>
      expect(useWorkspaceStore.getState().vaultPath).toBe("/tmp/paste-vault"),
    );
    expect(scaffoldedRef.current).toBe("/tmp/paste-vault");
    expect(writtenRef.current?.vaultPath).toBe("/tmp/paste-vault");
    expect(writtenRef.current?.filePath).toBe("runbooks/untitled.md");
    expect(writtenRef.current?.content).toContain(
      "GET https://api.example.com/users",
    );
  });

  it("V1 cenário 5 audit — consecutive errors across all three cards never crash the app", async () => {
    const user = userEvent.setup();
    const openVaultSpy = vi.fn(async () => {
      throw new Error("vault not a git repo");
    });
    useWorkspaceStore.setState({ openVault: openVaultSpy });
    mockTauriCommand("clone_vault_cmd", () => {
      throw new Error("fatal: repository 'foo' not found");
    });
    mockTauriCommand("create_vault_cmd", () => {
      throw new Error("permission denied");
    });
    vi.mocked(openDialog).mockResolvedValue("/tmp/parent");

    renderWithProviders(<EmptyVaultScreen />);

    // 1. Open fails → global banner.
    await user.click(screen.getByTestId("open-vault-card"));
    await waitFor(() =>
      expect(screen.getByTestId("empty-vault-error").textContent).toContain(
        "vault not a git repo",
      ),
    );

    // 2. Clone fails → inline error on card; banner stays cleared.
    await user.click(screen.getByTestId("clone-vault-expand"));
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "https://nope.invalid/x.git",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-error").textContent).toContain(
        "fatal",
      ),
    );

    // 3. Create fails → inline error on card.
    await user.click(screen.getByTestId("create-vault-expand"));
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-parent").textContent).toContain(
        "/tmp/parent",
      ),
    );
    await user.type(screen.getByTestId("create-vault-name"), "anything");
    await user.click(screen.getByTestId("create-vault-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-error").textContent).toContain(
        "permission denied",
      ),
    );

    // App alive: all three cards still rendered, store still null.
    expect(screen.getByTestId("open-vault-card")).toBeInTheDocument();
    expect(screen.getByTestId("clone-vault-card")).toBeInTheDocument();
    expect(screen.getByTestId("create-vault-card")).toBeInTheDocument();
    expect(useWorkspaceStore.getState().vaultPath).toBeNull();
  });

  it("Pasting non-URL text falls through (no scaffold)", async () => {
    let scaffoldCalled = false;
    mockTauriCommand("scaffold_vault", () => {
      scaffoldCalled = true;
      return null;
    });

    renderWithProviders(<EmptyVaultScreen />);

    const paste = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(paste, "clipboardData", {
      value: {
        getData: () => "hello world",
      },
    });
    document.dispatchEvent(paste);

    await new Promise((r) => setTimeout(r, 10));
    expect(scaffoldCalled).toBe(false);
  });
});
