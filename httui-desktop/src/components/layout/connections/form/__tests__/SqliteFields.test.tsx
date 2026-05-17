import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { SqliteFields } from "@/components/layout/connections/form/SqliteFields";

describe("SqliteFields", () => {
  it("renders the file-path input with the supplied value", () => {
    renderWithProviders(
      <SqliteFields dbName="/tmp/notes.db" onDbNameChange={vi.fn()} />,
    );
    const input = screen.getByPlaceholderText(
      "/path/to/database.db",
    ) as HTMLInputElement;
    expect(input.value).toBe("/tmp/notes.db");
  });

  it("dispatches onDbNameChange as the user types", async () => {
    const onChange = vi.fn();
    renderWithProviders(<SqliteFields dbName="" onDbNameChange={onChange} />);
    const input = screen.getByPlaceholderText("/path/to/database.db");
    await userEvent.setup().type(input, "x");
    expect(onChange).toHaveBeenCalled();
  });

  it("Browse button opens the OS dialog and writes the selected path through", async () => {
    vi.mocked(openDialog).mockResolvedValue("/picked/path.db");
    const onChange = vi.fn();
    renderWithProviders(<SqliteFields dbName="" onDbNameChange={onChange} />);
    await userEvent.setup().click(screen.getByLabelText("Browse"));
    // Wait a microtask so the awaited dialog resolves.
    await new Promise((r) => setTimeout(r, 10));
    expect(onChange).toHaveBeenCalledWith("/picked/path.db");
  });

  it("Browse cancellation is a no-op (no onChange call)", async () => {
    vi.mocked(openDialog).mockResolvedValue(null);
    const onChange = vi.fn();
    renderWithProviders(<SqliteFields dbName="" onDbNameChange={onChange} />);
    await userEvent.setup().click(screen.getByLabelText("Browse"));
    await new Promise((r) => setTimeout(r, 10));
    expect(onChange).not.toHaveBeenCalled();
  });
});
