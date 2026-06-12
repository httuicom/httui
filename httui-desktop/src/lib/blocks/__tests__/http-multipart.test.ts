import { describe, it, expect } from "vitest";
import {
  buildBinaryFileBody,
  inferContentType,
  isBinaryFileBody,
  parseMultipartBody,
  stringifyMultipartBody,
  type MultipartPart,
} from "../http-body-modes";

describe("inferContentType", () => {
  it("recognises common image types", () => {
    expect(inferContentType("avatar.png")).toBe("image/png");
    expect(inferContentType("photo.JPG")).toBe("image/jpeg");
    expect(inferContentType("photo.jpeg")).toBe("image/jpeg");
    expect(inferContentType("art.svg")).toBe("image/svg+xml");
  });

  it("recognises text-ish types", () => {
    expect(inferContentType("notes.md")).toBe("text/plain");
    expect(inferContentType("data.json")).toBe("application/json");
    expect(inferContentType("config.xml")).toBe("application/xml");
  });

  it("falls back to octet-stream for unknown extensions", () => {
    expect(inferContentType("payload.bin")).toBe("application/octet-stream");
    expect(inferContentType("noextension")).toBe("application/octet-stream");
    expect(inferContentType("")).toBe("application/octet-stream");
  });
});

describe("isBinaryFileBody", () => {
  it("matches a single `< /path` line", () => {
    expect(isBinaryFileBody("< /Users/joao/avatar.png")).toEqual({
      path: "/Users/joao/avatar.png",
    });
    expect(isBinaryFileBody("  < /tmp/data.bin  ")).toEqual({
      path: "/tmp/data.bin",
    });
  });

  it("rejects missing prefix or empty path", () => {
    expect(isBinaryFileBody("/Users/joao/avatar.png")).toBeNull();
    expect(isBinaryFileBody("< ")).toBeNull();
    expect(isBinaryFileBody("")).toBeNull();
  });

  it("rejects multi-line bodies (mixed content)", () => {
    expect(isBinaryFileBody("< /tmp/x\nsome other text")).toBeNull();
  });

  it("tolerates trailing newlines on a single-line body", () => {
    expect(isBinaryFileBody("< /tmp/x\n")).toEqual({ path: "/tmp/x" });
    expect(isBinaryFileBody("< /tmp/x\n\n  \n")).toEqual({ path: "/tmp/x" });
  });
});

describe("buildBinaryFileBody", () => {
  it("emits the canonical single-line shape", () => {
    expect(buildBinaryFileBody("/Users/joao/avatar.png")).toBe(
      "< /Users/joao/avatar.png",
    );
  });
});

describe("stringifyMultipartBody / parseMultipartBody (KV format)", () => {
  const parts: MultipartPart[] = [
    {
      kind: "text",
      name: "username",
      value: "alice",
      enabled: true,
    },
    {
      kind: "file",
      name: "avatar",
      value: "/Users/joao/Pictures/avatar.png",
      enabled: true,
    },
    {
      kind: "text",
      name: "bio",
      value: "hello world",
      enabled: true,
    },
  ];

  it("emits one line per part in canonical KV format", () => {
    const { body } = stringifyMultipartBody(parts);
    expect(body).toBe(
      [
        "username=alice",
        "avatar=< /Users/joao/Pictures/avatar.png",
        "bio=hello world",
      ].join("\n"),
    );
  });

  it("roundtrips parse → stringify → parse", () => {
    const { body } = stringifyMultipartBody(parts);
    const reparsed = parseMultipartBody(body);
    expect(reparsed.length).toBe(3);
    expect(reparsed[0]).toMatchObject({
      kind: "text",
      name: "username",
      value: "alice",
      enabled: true,
    });
    expect(reparsed[1]).toMatchObject({
      kind: "file",
      name: "avatar",
      value: "/Users/joao/Pictures/avatar.png",
      enabled: true,
      filename: "avatar.png",
      contentType: "image/png",
    });
    expect(reparsed[2]).toMatchObject({
      kind: "text",
      name: "bio",
      value: "hello world",
      enabled: true,
    });
  });

  it("is idempotent (second emit equals first)", () => {
    const a = stringifyMultipartBody(parts);
    const b = stringifyMultipartBody(parseMultipartBody(a.body));
    expect(b.body).toBe(a.body);
  });

  it("preserves disabled parts via `# ` prefix", () => {
    const withDisabled: MultipartPart[] = [
      { kind: "text", name: "name", value: "alice", enabled: true },
      { kind: "text", name: "secret", value: "hidden", enabled: false },
    ];
    const { body } = stringifyMultipartBody(withDisabled);
    expect(body).toBe("name=alice\n# secret=hidden");

    const reparsed = parseMultipartBody(body);
    expect(reparsed.length).toBe(2);
    expect(reparsed[0].enabled).toBe(true);
    expect(reparsed[1].enabled).toBe(false);
    expect(reparsed[1].name).toBe("secret");
    expect(reparsed[1].value).toBe("hidden");
  });

  it("preserves descriptions across roundtrip", () => {
    const withDesc: MultipartPart[] = [
      {
        kind: "text",
        name: "name",
        value: "alice",
        enabled: true,
        description: "user display name",
      },
    ];
    const { body } = stringifyMultipartBody(withDesc);
    expect(body).toBe("# desc: user display name\nname=alice");
    const reparsed = parseMultipartBody(body);
    expect(reparsed[0].description).toBe("user display name");
  });

  it("returns empty array when body is empty", () => {
    expect(parseMultipartBody("")).toEqual([]);
    expect(parseMultipartBody("   \n\n  ")).toEqual([]);
  });

  it("ignores free-form comment lines", () => {
    const body =
      "# this is a free-form comment\nname=alice\n# another comment\navatar=< /tmp/x.png";
    const parsed = parseMultipartBody(body);
    expect(parsed.length).toBe(2);
    expect(parsed[0].name).toBe("name");
    expect(parsed[1].name).toBe("avatar");
    expect(parsed[1].kind).toBe("file");
  });

  it("ignores lines without `=`", () => {
    expect(parseMultipartBody("just plain text").length).toBe(0);
    expect(parseMultipartBody("nokey").length).toBe(0);
  });

  it("treats an empty value as a text part with empty string", () => {
    const parsed = parseMultipartBody("empty=");
    expect(parsed.length).toBe(1);
    expect(parsed[0]).toMatchObject({
      kind: "text",
      name: "empty",
      value: "",
      enabled: true,
    });
  });

  it("file part picks up content-type from filename extension on parse", () => {
    const parsed = parseMultipartBody("doc=< /tmp/report.pdf");
    expect(parsed[0]).toMatchObject({
      kind: "file",
      name: "doc",
      value: "/tmp/report.pdf",
      filename: "report.pdf",
      contentType: "application/pdf",
    });
  });

  it("disabled file part round-trips with `# ` prefix", () => {
    const original: MultipartPart[] = [
      {
        kind: "file",
        name: "avatar",
        value: "/tmp/avatar.png",
        enabled: false,
      },
    ];
    const { body } = stringifyMultipartBody(original);
    expect(body).toBe("# avatar=< /tmp/avatar.png");
    const reparsed = parseMultipartBody(body);
    expect(reparsed[0]).toMatchObject({
      kind: "file",
      name: "avatar",
      value: "/tmp/avatar.png",
      enabled: false,
      filename: "avatar.png",
      contentType: "image/png",
    });
  });

  it("handles empty parts list", () => {
    const { body } = stringifyMultipartBody([]);
    expect(body).toBe("");
    expect(parseMultipartBody(body)).toEqual([]);
  });
});
