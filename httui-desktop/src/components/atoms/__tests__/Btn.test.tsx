import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { Btn } from "@/components/atoms";

describe("Btn atom", () => {
  it("renders the label and dispatches onClick", async () => {
    const onClick = vi.fn();
    renderWithProviders(<Btn onClick={onClick}>Run</Btn>);

    const btn = screen.getByRole("button", { name: "Run" });
    await userEvent.setup().click(btn);

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("defaults to the primary variant", () => {
    renderWithProviders(<Btn>Save</Btn>);
    const btn = screen.getByRole("button", { name: "Save" });
    expect(btn.getAttribute("data-variant")).toBe("primary");
  });

  it("renders a ghost variant when requested", () => {
    renderWithProviders(<Btn variant="ghost">Cancel</Btn>);
    const btn = screen.getByRole("button", { name: "Cancel" });
    expect(btn.getAttribute("data-variant")).toBe("ghost");
  });

  it("respects the disabled prop (no click dispatch)", async () => {
    const onClick = vi.fn();
    renderWithProviders(
      <Btn disabled onClick={onClick}>
        Disabled
      </Btn>,
    );
    const btn = screen.getByRole("button", { name: "Disabled" });
    await userEvent
      .setup()
      .click(btn)
      .catch(() => {
        // some chakra builds throw on disabled click; we still expect 0 calls
      });
    expect(onClick).not.toHaveBeenCalled();
  });

  it("forwards a ref to the underlying <button>", () => {
    let captured: HTMLButtonElement | null = null;
    renderWithProviders(
      <Btn
        ref={(el) => {
          captured = el;
        }}
      >
        Ref
      </Btn>,
    );
    expect(captured).toBeInstanceOf(HTMLButtonElement);
  });
});
