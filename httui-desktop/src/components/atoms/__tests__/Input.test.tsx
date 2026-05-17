import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { Input } from "@/components/atoms";

describe("Input atom", () => {
  it("renders an <input> element with the supplied placeholder", () => {
    renderWithProviders(<Input placeholder="Buscar key, valor, scope…" />);
    const node = screen.getByPlaceholderText("Buscar key, valor, scope…");
    expect(node.tagName).toBe("INPUT");
  });

  it("tags itself with data-atom='input' for testing/styling hooks", () => {
    renderWithProviders(<Input aria-label="search" />);
    const node = screen.getByLabelText("search");
    expect(node.getAttribute("data-atom")).toBe("input");
  });

  it("dispatches onChange as the user types", async () => {
    const onChange = vi.fn();
    renderWithProviders(<Input aria-label="search" onChange={onChange} />);
    const node = screen.getByLabelText("search");
    await userEvent.setup().type(node, "hi");
    expect(onChange).toHaveBeenCalled();
    expect((node as HTMLInputElement).value).toBe("hi");
  });

  it("forwards a ref to the underlying <input>", () => {
    let captured: HTMLInputElement | null = null;
    renderWithProviders(
      <Input
        aria-label="r"
        ref={(el) => {
          captured = el;
        }}
      />,
    );
    expect(captured).toBeInstanceOf(HTMLInputElement);
  });

  it("respects the disabled prop", async () => {
    const onChange = vi.fn();
    renderWithProviders(<Input aria-label="d" disabled onChange={onChange} />);
    const node = screen.getByLabelText("d") as HTMLInputElement;
    expect(node.disabled).toBe(true);
  });
});
