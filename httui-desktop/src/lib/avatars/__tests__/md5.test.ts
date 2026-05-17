import { describe, expect, it } from "vitest";

import { md5 } from "@/lib/avatars/md5";

describe("md5", () => {
  // RFC-1321 test vectors.
  it("hashes the empty string", () => {
    expect(md5("")).toBe("d41d8cd98f00b204e9800998ecf8427e");
  });

  it("hashes a single ASCII char", () => {
    expect(md5("a")).toBe("0cc175b9c0f1b6a831c399e269772661");
  });

  it("hashes a short ASCII string", () => {
    expect(md5("abc")).toBe("900150983cd24fb0d6963f7d28e17f72");
  });

  it("hashes the message-digest ASCII vector", () => {
    expect(md5("message digest")).toBe("f96b697d7cb7938d525a2f31aaf161d0");
  });

  it("hashes the lowercase alphabet", () => {
    expect(md5("abcdefghijklmnopqrstuvwxyz")).toBe(
      "c3fcd3d76192e4007dfb496cca67e13b",
    );
  });

  it("hashes a known-Gravatar email vector", () => {
    // Canonical example from gravatar.com/site/implement/hash/.
    expect(md5("MyEmailAddress@example.com".toLowerCase().trim())).toBe(
      "0bc83cb571cd1c50ba6f3e8a78ef1346",
    );
  });

  it("handles UTF-8 input", () => {
    // "café" — multi-byte. Reference value computed via Node's
    // `crypto.createHash('md5').update('café', 'utf8').digest('hex')`.
    expect(md5("café")).toBe("07117fe4a1ebd544965dc19573183da2");
  });
});
