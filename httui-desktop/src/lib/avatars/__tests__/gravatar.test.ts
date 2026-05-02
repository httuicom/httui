import { describe, expect, it } from "vitest";

import { gravatarUrl } from "@/lib/avatars/gravatar";

describe("gravatarUrl", () => {
  it("returns null for empty / null / whitespace input", () => {
    expect(gravatarUrl(null)).toBeNull();
    expect(gravatarUrl(undefined)).toBeNull();
    expect(gravatarUrl("")).toBeNull();
    expect(gravatarUrl("   ")).toBeNull();
  });

  it("hashes the lower-cased trimmed email", () => {
    // Canonical example from gravatar.com/site/implement/hash/.
    const url = gravatarUrl(" MyEmailAddress@example.com ");
    expect(url).toContain("0bc83cb571cd1c50ba6f3e8a78ef1346");
  });

  it("requests 2× the requested size for retina", () => {
    const url = gravatarUrl("a@b.com", { size: 20 });
    expect(url).toContain("s=40");
  });

  it("uses 404 fallback by default so consumers can show their own", () => {
    const url = gravatarUrl("a@b.com");
    expect(url).toContain("d=404");
  });

  it("supports an alternate fallback strategy", () => {
    const url = gravatarUrl("a@b.com", { fallback: "identicon" });
    expect(url).toContain("d=identicon");
    expect(url).not.toContain("d=404");
  });
});
