import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { QuickOpen } from "@/components/search/QuickOpen";
import { renderWithWorkspace, screen } from "@/test/render";
import { clearTauriMocks } from "@/test/mocks/tauri";

type SearchResult = { path: string; name: string };

const mockState = {
  query: "",
  results: [] as SearchResult[],
  safeIndex: 0,
  setSelectedIndex: vi.fn(),
  handleSearch: vi.fn(),
  handleSelect: vi.fn(),
  handleKeyDown: vi.fn(),
};

vi.mock("@/hooks/useFileSearch", () => ({
  useFileSearch: () => mockState,
}));

function reset(over: Partial<typeof mockState> = {}) {
  mockState.query = "";
  mockState.results = [];
  mockState.safeIndex = 0;
  mockState.setSelectedIndex = vi.fn();
  mockState.handleSearch = vi.fn();
  mockState.handleSelect = vi.fn();
  mockState.handleKeyDown = vi.fn();
  Object.assign(mockState, over);
}

describe("QuickOpen", () => {
  beforeEach(() => {
    reset();
    clearTauriMocks();
    // jsdom doesn't implement scrollIntoView; QuickOpen invokes it
    // when the selected index changes.
    if (!HTMLElement.prototype.scrollIntoView) {
      HTMLElement.prototype.scrollIntoView = vi.fn();
    }
  });

  afterEach(() => {
    clearTauriMocks();
  });

  it("renders nothing when closed", () => {
    const { container } = renderWithWorkspace(
      <QuickOpen open={false} onClose={vi.fn()} />,
    );
    expect(container.querySelector("input")).toBeNull();
  });

  it("renders the search input when open", () => {
    renderWithWorkspace(<QuickOpen open onClose={vi.fn()} />);
    expect(
      screen.getByPlaceholderText("Buscar arquivo… ou #tag"),
    ).toBeInTheDocument();
  });

  it('shows "Nenhum resultado" when query has no matches', () => {
    reset({ query: "xyz", results: [] });
    renderWithWorkspace(<QuickOpen open onClose={vi.fn()} />);
    expect(screen.getByText("Nenhum resultado")).toBeInTheDocument();
  });

  it("renders matching results with name + path", () => {
    reset({
      results: [
        { path: "runbooks/login.md", name: "login.md" },
        { path: "runbooks/auth.md", name: "auth.md" },
      ],
    });
    renderWithWorkspace(<QuickOpen open onClose={vi.fn()} />);
    expect(screen.getByText("login.md")).toBeInTheDocument();
    expect(screen.getByText("runbooks/login.md")).toBeInTheDocument();
    expect(screen.getByText("auth.md")).toBeInTheDocument();
  });

  it("clicking a result calls handleSelect with its index", async () => {
    const user = userEvent.setup();
    reset({
      results: [
        { path: "runbooks/login.md", name: "login.md" },
        { path: "runbooks/auth.md", name: "auth.md" },
      ],
    });
    renderWithWorkspace(<QuickOpen open onClose={vi.fn()} />);
    await user.click(screen.getByText("auth.md"));
    expect(mockState.handleSelect).toHaveBeenCalledWith(1);
  });

  it("hovering a result updates the selected index", async () => {
    const user = userEvent.setup();
    reset({
      results: [
        { path: "runbooks/login.md", name: "login.md" },
        { path: "runbooks/auth.md", name: "auth.md" },
      ],
    });
    renderWithWorkspace(<QuickOpen open onClose={vi.fn()} />);
    await user.hover(screen.getByText("auth.md"));
    expect(mockState.setSelectedIndex).toHaveBeenCalledWith(1);
  });

  it("typing in the input dispatches handleSearch", async () => {
    const user = userEvent.setup();
    renderWithWorkspace(<QuickOpen open onClose={vi.fn()} />);
    await user.type(
      screen.getByPlaceholderText("Buscar arquivo… ou #tag"),
      "lo",
    );
    expect(mockState.handleSearch).toHaveBeenCalled();
  });
});
