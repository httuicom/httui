import { describe, it, expect } from "vitest";
import {
  isPrerelease,
  shouldOfferUpdate,
} from "@/lib/updater/should-offer-update";

describe("isPrerelease", () => {
  it.each([
    "1.0.0-rc1",
    "1.0.0-rc.1",
    "1.0.0-rc.12",
    "2.3.4-beta2",
    "2.3.4-beta.2",
    "0.9.0-alpha1",
    "0.9.0-alpha.10",
    "1.0.0-RC.1",
  ])("treats %s as a pre-release", (v) => {
    expect(isPrerelease(v)).toBe(true);
  });

  it.each(["1.0.0", "2.3.4", "10.20.30", "1.0.0-rc", "1.0.0+build.5"])(
    "treats %s as stable",
    (v) => {
      expect(isPrerelease(v)).toBe(false);
    },
  );
});

describe("shouldOfferUpdate", () => {
  it("returns false when there is no version", () => {
    expect(shouldOfferUpdate(null, false)).toBe(false);
    expect(shouldOfferUpdate(undefined, true)).toBe(false);
    expect(shouldOfferUpdate("", true)).toBe(false);
  });

  it("offers stable releases regardless of the opt-in", () => {
    expect(shouldOfferUpdate("1.2.0", false)).toBe(true);
    expect(shouldOfferUpdate("1.2.0", true)).toBe(true);
  });

  it("hides pre-releases when the user has not opted in", () => {
    expect(shouldOfferUpdate("1.2.0-rc.1", false)).toBe(false);
    expect(shouldOfferUpdate("1.2.0-beta2", false)).toBe(false);
  });

  it("offers pre-releases once the user opts in", () => {
    expect(shouldOfferUpdate("1.2.0-rc.1", true)).toBe(true);
    expect(shouldOfferUpdate("1.2.0-alpha.3", true)).toBe(true);
  });
});
