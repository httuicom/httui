import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import userEvent from "@testing-library/user-event";

import { StatusBar } from "@/components/layout/StatusBar";
import { useEnvironmentStore } from "@/stores/environment";
import { usePendingSecretsStore } from "@/stores/pendingSecrets";
import { useWorkspaceStore } from "@/stores/workspace";
import type { MissingRef } from "@/lib/tauri/commands";

const PENDING_REF: MissingRef = {
  source_file: "connections.toml",
  label: "postgres-prod",
  keychain_key: "conn:abc:password",
  kind: "connection",
};

vi.mock("@/lib/theme/apply", () => ({ applyTheme: vi.fn() }));

const mkEnv = (id: string, name: string) => ({
  id,
  name,
  is_active: true,
  created_at: "2026-01-01T00:00:00Z",
});

beforeEach(() => {
  clearTauriMocks();
  mockTauriCommand("git_status_cmd", () => ({
    branch: "main",
    upstream: null,
    ahead: 0,
    behind: 0,
    changed: [],
    clean: true,
  }));
  useWorkspaceStore.setState({
    vaultPath: null,
    activeConnection: null,
  } as never);
  useEnvironmentStore.setState({
    environments: [],
    activeEnvironment: null,
    switchEnvironment: vi.fn(),
  } as never);
  usePendingSecretsStore.getState().reset();
});

afterEach(() => {
  clearTauriMocks();
});

describe("StatusBar", () => {
  it("renders inside a 22px-tall mono shell", () => {
    renderWithProviders(<StatusBar />);
    const shell = screen.getByTestId("status-bar");
    expect(shell.getAttribute("data-atom")).toBe("statusbar");
  });

  it("shows '—' branch placeholder when no vault is open", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.getByTestId("status-branch").textContent).toBe("—");
  });

  it("renders 'no env' when no active environment", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.getByTestId("status-env").textContent).toContain("no env");
  });

  it("env Dot variant is err for prod*", () => {
    useEnvironmentStore.setState({
      environments: [mkEnv("a", "prod-canary")],
      activeEnvironment: mkEnv("a", "prod-canary"),
      switchEnvironment: vi.fn(),
    } as never);
    renderWithProviders(<StatusBar />);
    const dot = screen
      .getByTestId("status-env")
      .querySelector('[data-atom="dot"]');
    expect(dot?.getAttribute("data-variant")).toBe("err");
  });

  it("env Dot variant is warn for staging", () => {
    useEnvironmentStore.setState({
      environments: [mkEnv("b", "staging")],
      activeEnvironment: mkEnv("b", "staging"),
      switchEnvironment: vi.fn(),
    } as never);
    renderWithProviders(<StatusBar />);
    const dot = screen
      .getByTestId("status-env")
      .querySelector('[data-atom="dot"]');
    expect(dot?.getAttribute("data-variant")).toBe("warn");
  });

  it("env Dot variant is ok for local-style names", () => {
    useEnvironmentStore.setState({
      environments: [mkEnv("c", "local")],
      activeEnvironment: mkEnv("c", "local"),
      switchEnvironment: vi.fn(),
    } as never);
    renderWithProviders(<StatusBar />);
    const dot = screen
      .getByTestId("status-env")
      .querySelector('[data-atom="dot"]');
    expect(dot?.getAttribute("data-variant")).toBe("ok");
  });

  it("hides the connection cell when no connection is active", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.queryByTestId("status-conn")).toBeNull();
  });

  it("renders the connection name + ok dot when a connection is active", () => {
    useWorkspaceStore.setState({
      vaultPath: null,
      activeConnection: { name: "pg-prod", status: "connected" },
    } as never);
    renderWithProviders(<StatusBar />);
    const cell = screen.getByTestId("status-conn");
    expect(cell.textContent).toContain("pg-prod");
    expect(
      cell.querySelector('[data-atom="dot"]')?.getAttribute("data-variant"),
    ).toBe("ok");
  });

  it("renders cursor position from props", () => {
    renderWithProviders(<StatusBar cursorLine={12} cursorCol={4} />);
    expect(screen.getByTestId("status-cursor").textContent).toBe(
      "Ln 12, Col 4",
    );
  });

  it("encoding is UTF-8 (static)", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.getByTestId("status-encoding").textContent).toBe("UTF-8");
  });

  it("chained indicator hidden by default", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.queryByTestId("status-chained")).toBeNull();
  });

  it("chained indicator visible when chained=true", () => {
    renderWithProviders(<StatusBar chained />);
    expect(screen.getByTestId("status-chained")).toBeInTheDocument();
  });

  it("renders the version pill", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.getByTestId("status-version").textContent).toMatch(
      /^v[\w.-]+/,
    );
  });

  it("categorises gitStatus.changed into +N ~M -D buckets", async () => {
    useWorkspaceStore.setState({ vaultPath: "/v" } as never);
    mockTauriCommand("git_status_cmd", () => ({
      branch: "main",
      upstream: null,
      ahead: 0,
      behind: 0,
      changed: [
        // Two added (one staged "A", one untracked "??")
        { path: "new1.md", status: "A", staged: true, untracked: false },
        { path: "new2.md", status: "??", staged: false, untracked: true },
        // Three modified (M / R / C codes all collapse to "modified")
        { path: "doc.md", status: "M", staged: false, untracked: false },
        { path: "renamed.md", status: "R", staged: false, untracked: false },
        { path: "copied.md", status: "C", staged: false, untracked: false },
        // One deleted
        { path: "old.md", status: "D", staged: false, untracked: false },
      ],
      clean: false,
    }));
    renderWithProviders(<StatusBar />);
    await new Promise((r) => setTimeout(r, 0));
    const counts = await screen.findByTestId("status-changes");
    expect(counts.textContent).toContain("+2");
    expect(counts.textContent).toContain("~3");
    expect(counts.textContent).toContain("-1");
  });
});

describe("StatusBar — pending secrets badge", () => {
  it("hides the badge when there are no pending secrets", () => {
    renderWithProviders(<StatusBar />);
    expect(screen.queryByTestId("status-pending-secrets")).toBeNull();
  });

  it("hides the badge while the modal is currently open", () => {
    usePendingSecretsStore.getState().setPending([PENDING_REF]);
    renderWithProviders(<StatusBar />);
    expect(screen.queryByTestId("status-pending-secrets")).toBeNull();
  });

  it("shows the badge with singular label after dismiss", () => {
    usePendingSecretsStore.getState().setPending([PENDING_REF]);
    usePendingSecretsStore.getState().dismiss();
    renderWithProviders(<StatusBar />);
    expect(
      screen.getByTestId("status-pending-secrets").textContent,
    ).toContain("1 secret pendente");
  });

  it("shows pluralized label for 2+ pending refs", () => {
    usePendingSecretsStore.getState().setPending([
      PENDING_REF,
      {
        ...PENDING_REF,
        keychain_key: "env:local:STRIPE_KEY",
        label: "STRIPE_KEY",
        kind: "env",
      },
    ]);
    usePendingSecretsStore.getState().dismiss();
    renderWithProviders(<StatusBar />);
    expect(
      screen.getByTestId("status-pending-secrets").textContent,
    ).toContain("2 secrets pendentes");
  });

  it("clicking the badge reopens the modal", async () => {
    const user = userEvent.setup();
    usePendingSecretsStore.getState().setPending([PENDING_REF]);
    usePendingSecretsStore.getState().dismiss();
    renderWithProviders(<StatusBar />);
    expect(usePendingSecretsStore.getState().modalOpen).toBe(false);
    await user.click(screen.getByTestId("status-pending-secrets"));
    expect(usePendingSecretsStore.getState().modalOpen).toBe(true);
  });
});
