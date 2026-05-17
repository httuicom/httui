import { describe, expect, it } from "vitest";

import { parseRemoteUrl, type ParsedRemote } from "../remote-host";
import {
  composeBlobUrl,
  composeCompareUrl,
  composeTreeUrl,
} from "../share-url";

function parsed(url: string): ParsedRemote {
  const p = parseRemoteUrl(url);
  if (!p) throw new Error(`expected parse for ${url}`);
  return p;
}

describe("composeBlobUrl", () => {
  it("composes a GitHub blob URL", () => {
    const r = composeBlobUrl(
      parsed("https://github.com/owner/repo.git"),
      "abc1234",
      "src/lib/foo.ts",
    );
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.url).toBe(
        "https://github.com/owner/repo/blob/abc1234/src/lib/foo.ts",
      );
    }
  });

  it("composes a GitLab blob URL with /-/blob/ shape", () => {
    const r = composeBlobUrl(
      parsed("https://gitlab.com/group/repo.git"),
      "abc",
      "src/x",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://gitlab.com/group/repo/-/blob/abc/src/x");
  });

  it("composes a self-hosted GitLab blob URL", () => {
    const r = composeBlobUrl(
      parsed("git@gitlab.example.com:group/repo.git"),
      "sha",
      "p",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://gitlab.example.com/group/repo/-/blob/sha/p");
  });

  it("appends #L<line> when line is provided", () => {
    const r = composeBlobUrl(
      parsed("https://github.com/owner/repo"),
      "abc",
      "x.ts",
      42,
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url.endsWith("#L42")).toBe(true);
  });

  it("ignores non-positive lines", () => {
    const r = composeBlobUrl(
      parsed("https://github.com/owner/repo"),
      "abc",
      "x.ts",
      0,
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).not.toContain("#L");
  });

  it("strips leading slashes from the file path", () => {
    const r = composeBlobUrl(
      parsed("https://github.com/owner/repo"),
      "abc",
      "/src/x",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://github.com/owner/repo/blob/abc/src/x");
  });

  it("returns unsupported-host hint for Bitbucket / Gitea / Other", () => {
    for (const url of [
      "git@bitbucket.org:t/r.git",
      "https://gitea.com/o/r",
      "https://code.example.com/o/r",
    ]) {
      const r = composeBlobUrl(parsed(url), "abc", "x");
      expect(r.ok).toBe(false);
      if (!r.ok) {
        expect(r.reason).toBe("unsupported-host");
        expect(r.hint).toMatch(/Manual: open/);
        expect(r.fallback.startsWith("https://")).toBe(true);
      }
    }
  });

  it("converts ssh URLs to https for the rendered share link", () => {
    const r = composeBlobUrl(
      parsed("git@github.com:owner/repo.git"),
      "abc",
      "x",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url.startsWith("https://github.com/")).toBe(true);
  });
});

describe("composeTreeUrl", () => {
  it("composes a GitHub tree URL", () => {
    const r = composeTreeUrl(
      parsed("https://github.com/owner/repo"),
      "abc1234",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://github.com/owner/repo/tree/abc1234");
  });

  it("composes a GitLab tree URL with /-/tree/ shape", () => {
    const r = composeTreeUrl(parsed("https://gitlab.com/g/r"), "abc");
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://gitlab.com/g/r/-/tree/abc");
  });

  it("returns the manual hint for unsupported forges", () => {
    const r = composeTreeUrl(parsed("https://code.example.com/o/r"), "abc");
    expect(r.ok).toBe(false);
  });
});

describe("composeCompareUrl", () => {
  it("composes a GitHub compare URL", () => {
    const r = composeCompareUrl(
      parsed("https://github.com/owner/repo"),
      "main",
      "feat/x",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://github.com/owner/repo/compare/main...feat/x");
  });

  it("composes a GitLab compare URL with /-/compare/ shape", () => {
    const r = composeCompareUrl(
      parsed("https://gitlab.com/g/r"),
      "main",
      "feat/x",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://gitlab.com/g/r/-/compare/main...feat/x");
  });

  it("composes a self-hosted GitLab compare URL", () => {
    const r = composeCompareUrl(
      parsed("git@gitlab.example.com:g/r.git"),
      "main",
      "x",
    );
    if (!r.ok) throw new Error("expected ok");
    expect(r.url).toBe("https://gitlab.example.com/g/r/-/compare/main...x");
  });

  it("returns the manual hint for Bitbucket / Gitea / Other", () => {
    const r = composeCompareUrl(parsed("git@bitbucket.org:t/r.git"), "a", "b");
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.fallback).toBe("https://bitbucket.org/t/r");
    }
  });
});
