import { beforeEach, describe, expect, it } from "vitest";

import { useEnvSwitcherStore } from "@/stores/envSwitcher";

describe("useEnvSwitcherStore", () => {
  beforeEach(() => {
    useEnvSwitcherStore.setState({ open: false });
  });

  it("starts closed", () => {
    expect(useEnvSwitcherStore.getState().open).toBe(false);
  });

  it("openSwitcher sets open=true", () => {
    useEnvSwitcherStore.getState().openSwitcher();
    expect(useEnvSwitcherStore.getState().open).toBe(true);
  });

  it("closeSwitcher sets open=false", () => {
    useEnvSwitcherStore.getState().openSwitcher();
    useEnvSwitcherStore.getState().closeSwitcher();
    expect(useEnvSwitcherStore.getState().open).toBe(false);
  });

  it("setOpen mirrors the boolean (controlled bridge)", () => {
    useEnvSwitcherStore.getState().setOpen(true);
    expect(useEnvSwitcherStore.getState().open).toBe(true);
    useEnvSwitcherStore.getState().setOpen(false);
    expect(useEnvSwitcherStore.getState().open).toBe(false);
  });
});
