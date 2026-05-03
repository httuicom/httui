import { describe, it, expect } from "vitest";

import {
  VARIABLE_HELPERS,
  VARIABLE_SCOPES,
  VARIABLE_SCOPE_META,
  VAR_RESOLUTION_HINT,
  type VariableScope,
} from "@/components/layout/variables/variable-scopes";

describe("variable-scopes metadata", () => {
  it("exports the 5 canvas scopes in order", () => {
    expect(VARIABLE_SCOPES).toEqual([
      "all",
      "workspace",
      "captured",
      "secret",
      "personal",
    ]);
  });

  it("each scope has matching meta with id/label/icon/hint", () => {
    for (const scope of VARIABLE_SCOPES) {
      const meta = VARIABLE_SCOPE_META[scope];
      expect(meta).toBeDefined();
      expect(meta.id).toBe(scope);
      expect(meta.label.length).toBeGreaterThan(0);
      expect(typeof meta.icon).toBe("function");
      expect(meta.hint.length).toBeGreaterThan(0);
    }
  });

  it("all scope ids are unique", () => {
    const ids = VARIABLE_SCOPES.map((s) => s as VariableScope);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("ships the canvas resolution chain hint", () => {
    expect(VAR_RESOLUTION_HINT).toMatch(/block.*env.*workspace.*secret/);
  });

  it("exports 4 helper functions per canvas spec", () => {
    expect(VARIABLE_HELPERS).toHaveLength(4);
    expect(VARIABLE_HELPERS.map((h) => h.syntax)).toEqual([
      "{{uuid()}}",
      "{{now()}}",
      "{{base64(x)}}",
      "{{$prev.body.id}}",
    ]);
    for (const h of VARIABLE_HELPERS) {
      expect(h.hint.length).toBeGreaterThan(0);
    }
  });
});
