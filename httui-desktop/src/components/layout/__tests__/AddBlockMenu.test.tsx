import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import {
  AddBlockMenu,
  BLOCK_TEMPLATES,
  type BlockKind,
} from "@/components/layout/AddBlockMenu";

describe("AddBlockMenu", () => {
  it("renders a single trigger with aria-label='Add block' by default", () => {
    renderWithProviders(<AddBlockMenu onInsert={() => {}} />);
    expect(
      screen.getByRole("button", { name: "Add block" }),
    ).toBeInTheDocument();
  });

  it("supports an ariaLabel override", () => {
    renderWithProviders(
      <AddBlockMenu onInsert={() => {}} ariaLabel="Insert here" />,
    );
    expect(
      screen.getByRole("button", { name: "Insert here" }),
    ).toBeInTheDocument();
  });

  it("opens a menu with all 7 block kinds", async () => {
    const user = userEvent.setup();
    renderWithProviders(<AddBlockMenu onInsert={() => {}} />);

    await user.click(screen.getByRole("button", { name: "Add block" }));

    const expectedKinds: BlockKind[] = [
      "markdown",
      "http",
      "sql",
      "mongodb",
      "websocket",
      "graphql",
      "shell",
    ];
    for (const kind of expectedKinds) {
      expect(screen.getByText(BLOCK_TEMPLATES[kind].label)).toBeInTheDocument();
    }
  });

  it("dispatches onInsert with the chosen template", async () => {
    const user = userEvent.setup();
    const onInsert = vi.fn();
    renderWithProviders(<AddBlockMenu onInsert={onInsert} />);

    await user.click(screen.getByRole("button", { name: "Add block" }));
    await user.click(screen.getByText("HTTP"));

    expect(onInsert).toHaveBeenCalledTimes(1);
    expect(onInsert.mock.calls[0][0]).toEqual(BLOCK_TEMPLATES.http);
  });

  it("non-executable kinds carry the executable=false marker in their template", () => {
    for (const k of [
      "mongodb",
      "websocket",
      "graphql",
      "shell",
    ] as BlockKind[]) {
      expect(BLOCK_TEMPLATES[k].executable).toBe(false);
      expect(BLOCK_TEMPLATES[k].insert).toContain("executable=false");
    }
  });

  it("executable kinds (HTTP + SQL) do NOT carry executable=false", () => {
    for (const k of ["http", "sql"] as BlockKind[]) {
      expect(BLOCK_TEMPLATES[k].executable).toBe(true);
      expect(BLOCK_TEMPLATES[k].insert).not.toContain("executable=false");
    }
  });

  it("HTTP template lands at the URL position via cursorOffset", () => {
    expect(BLOCK_TEMPLATES.http.cursorOffset).toBe(-5);
    expect(BLOCK_TEMPLATES.http.insert).toContain("```http alias=req1");
  });
});
