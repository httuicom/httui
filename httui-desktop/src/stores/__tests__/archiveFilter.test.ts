import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { useArchiveFilterStore } from "../archiveFilter";

beforeEach(() => {
  useArchiveFilterStore.setState({ showArchived: false });
});

afterEach(() => {
  // Persist middleware writes to localStorage; clean it so tests don't
  // bleed across runs.
  if (typeof localStorage !== "undefined") {
    localStorage.removeItem("archive-filter");
  }
});

describe("useArchiveFilterStore", () => {
  it("starts with showArchived = false (default state)", () => {
    expect(useArchiveFilterStore.getState().showArchived).toBe(false);
  });

  it("toggleShowArchived flips the boolean", () => {
    const s = useArchiveFilterStore.getState();
    s.toggleShowArchived();
    expect(useArchiveFilterStore.getState().showArchived).toBe(true);
    useArchiveFilterStore.getState().toggleShowArchived();
    expect(useArchiveFilterStore.getState().showArchived).toBe(false);
  });

  it("setShowArchived sets the value directly", () => {
    useArchiveFilterStore.getState().setShowArchived(true);
    expect(useArchiveFilterStore.getState().showArchived).toBe(true);
    useArchiveFilterStore.getState().setShowArchived(false);
    expect(useArchiveFilterStore.getState().showArchived).toBe(false);
  });
});
