import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useInlineForm, type InlineValidation } from "@/hooks/useInlineForm";

const required = (v: string): InlineValidation =>
  v.trim() ? { ok: true } : { ok: false, reason: "Name is required" };

describe("useInlineForm", () => {
  it("starts with the initial value, untouched, no surfaced error", () => {
    const { result } = renderHook(() => useInlineForm("", required));
    expect(result.current.value).toBe("");
    // invalid (empty) but not yet touched → nothing surfaced
    expect(result.current.showError).toBe(false);
    expect(result.current.error).toBe("Name is required");
  });

  it("seeds from a non-empty initial (Rename case)", () => {
    const { result } = renderHook(() => useInlineForm("prod", required));
    expect(result.current.value).toBe("prod");
    expect(result.current.showError).toBe(false);
    expect(result.current.error).toBeUndefined();
  });

  it("setValue updates the value and re-validates without surfacing until submit", () => {
    const { result } = renderHook(() => useInlineForm("ok", required));
    act(() => result.current.setValue("   "));
    expect(result.current.value).toBe("   ");
    expect(result.current.error).toBe("Name is required");
    // still untouched → not shown
    expect(result.current.showError).toBe(false);
  });

  it("attemptSubmit on an invalid value returns false and surfaces the error", () => {
    const { result } = renderHook(() => useInlineForm("", required));
    let ok!: boolean;
    act(() => {
      ok = result.current.attemptSubmit();
    });
    expect(ok).toBe(false);
    expect(result.current.showError).toBe(true);
    expect(result.current.error).toBe("Name is required");
  });

  it("attemptSubmit on a valid value returns true and never surfaces an error", () => {
    const { result } = renderHook(() => useInlineForm("staging", required));
    let ok!: boolean;
    act(() => {
      ok = result.current.attemptSubmit();
    });
    expect(ok).toBe(true);
    expect(result.current.showError).toBe(false);
    expect(result.current.error).toBeUndefined();
  });

  it("clears the surfaced error once the value becomes valid after a failed submit", () => {
    const { result } = renderHook(() => useInlineForm("", required));
    act(() => {
      result.current.attemptSubmit();
    });
    expect(result.current.showError).toBe(true);
    act(() => result.current.setValue("fixed"));
    // touched stays true, but now valid → no error shown
    expect(result.current.showError).toBe(false);
    expect(result.current.error).toBeUndefined();
  });

  it("calls the (already-curried) validator with the live value each render", () => {
    const validate = vi.fn(
      (v: string): InlineValidation =>
        v === "dupe" ? { ok: false, reason: "exists" } : { ok: true },
    );
    const { result } = renderHook(() => useInlineForm("a", validate));
    expect(validate).toHaveBeenCalledWith("a");
    act(() => result.current.setValue("dupe"));
    expect(validate).toHaveBeenLastCalledWith("dupe");
    expect(result.current.error).toBe("exists");
  });
});
