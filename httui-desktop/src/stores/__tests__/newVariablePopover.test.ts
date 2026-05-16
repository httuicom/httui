import { beforeEach, describe, expect, it } from "vitest";

import { useNewVariablePopoverStore } from "@/stores/newVariablePopover";

const s = () => useNewVariablePopoverStore.getState();

describe("useNewVariablePopoverStore", () => {
  beforeEach(() => useNewVariablePopoverStore.setState({ open: false }));

  it("starts closed", () => {
    expect(s().open).toBe(false);
  });

  it("openForm / closeForm toggle the flag", () => {
    s().openForm();
    expect(s().open).toBe(true);
    s().closeForm();
    expect(s().open).toBe(false);
  });

  it("setOpen mirrors the boolean", () => {
    s().setOpen(true);
    expect(s().open).toBe(true);
    s().setOpen(false);
    expect(s().open).toBe(false);
  });
});
