import { describe, expect, it } from "vitest";

import { system } from "@/lib/theme";
import {
  FONT_MARKDOWN_BODY,
  FONT_MONO,
  FONT_SANS,
  FONT_SERIF,
} from "@/theme/tokens";

// The Chakra system maps the raw token stacks from `@/theme/tokens` onto
// named font tokens that emit `--chakra-fonts-*` CSS vars. The editor and
// component tree consume those vars (e.g. editor-theme.ts uses
// `var(--chakra-fonts-markdown)` for markdown prose), so a regression here
// silently drops the wired font back to a fallback.
describe("Chakra font tokens", () => {
  it("wires markdown prose to FONT_MARKDOWN_BODY (Latin Modern Roman)", () => {
    expect(system.token("fonts.markdown")).toBe(FONT_MARKDOWN_BODY);
    expect(system.token("fonts.markdown")).toMatch(/^"Latin Modern Roman"/);
  });

  it("keeps body/heading/mono/serif mapped to their token stacks", () => {
    expect(system.token("fonts.body")).toBe(FONT_SANS);
    expect(system.token("fonts.heading")).toBe(FONT_SERIF);
    expect(system.token("fonts.mono")).toBe(FONT_MONO);
    expect(system.token("fonts.serif")).toBe(FONT_SERIF);
  });

  it("exposes the markdown token via the tokens map too", () => {
    expect(system.tokens.getByName("fonts.markdown")?.value).toBe(
      FONT_MARKDOWN_BODY,
    );
  });
});
