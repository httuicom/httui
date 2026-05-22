import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActiveFileSaver, saveActiveFileNow } from "@/lib/active-file-save";

describe("active-file-save", () => {
  beforeEach(() => {
    setActiveFileSaver(null);
  });

  it("should call the registered saver function", () => {
    const mockSaver = vi.fn();
    setActiveFileSaver(mockSaver);
    saveActiveFileNow();
    expect(mockSaver).toHaveBeenCalledOnce();
  });

  it("should handle null saver gracefully", () => {
    setActiveFileSaver(null);
    expect(() => saveActiveFileNow()).not.toThrow();
  });

  it("should replace the saver when setActiveFileSaver is called again", () => {
    const firstSaver = vi.fn();
    const secondSaver = vi.fn();

    setActiveFileSaver(firstSaver);
    saveActiveFileNow();
    expect(firstSaver).toHaveBeenCalledOnce();
    expect(secondSaver).not.toHaveBeenCalled();

    setActiveFileSaver(secondSaver);
    saveActiveFileNow();
    expect(firstSaver).toHaveBeenCalledOnce();
    expect(secondSaver).toHaveBeenCalledOnce();
  });
});
