import { describe, it, expect } from "vitest";

import {
  parseJsonForVisualize,
  shouldDefaultExpand,
  initialCollapsedPaths,
  flattenJson,
  primitiveDisplay,
  primitiveColor,
} from "@/components/blocks/http/fenced/HttpJsonVisualizer";

describe("parseJsonForVisualize", () => {
  it("parses object/array bodies", () => {
    expect(parseJsonForVisualize('{"a":1}')).toEqual({ a: 1 });
    expect(parseJsonForVisualize("  [1,2] ")).toEqual([1, 2]);
  });

  it("returns null for non-container or invalid JSON", () => {
    expect(parseJsonForVisualize('"a string"')).toBeNull();
    expect(parseJsonForVisualize("42")).toBeNull();
    expect(parseJsonForVisualize("{not json}")).toBeNull();
    expect(parseJsonForVisualize("")).toBeNull();
  });
});

describe("shouldDefaultExpand", () => {
  it("always expands the root", () => {
    expect(shouldDefaultExpand({ a: 1 }, 0)).toBe(true);
  });

  it("expands depth-1 containers with ≤ 20 children, collapses larger", () => {
    expect(shouldDefaultExpand([1, 2, 3], 1)).toBe(true);
    expect(shouldDefaultExpand(Array.from({ length: 21 }), 1)).toBe(false);
    expect(shouldDefaultExpand({ a: 1, b: 2 }, 1)).toBe(true);
    const big = Object.fromEntries(
      Array.from({ length: 21 }, (_, i) => [`k${i}`, i]),
    );
    expect(shouldDefaultExpand(big, 1)).toBe(false);
  });

  it("collapses anything at depth ≥ 2", () => {
    expect(shouldDefaultExpand({ a: 1 }, 2)).toBe(false);
    expect(shouldDefaultExpand("scalar", 1)).toBe(false);
  });
});

describe("initialCollapsedPaths", () => {
  it("collapses deep / oversized branches but not the shallow ones", () => {
    const data = {
      small: { x: 1 },
      deep: { level1: { level2: { z: 1 } } },
    };
    const collapsed = initialCollapsedPaths(data);
    // depth-1 small object stays expanded (≤20 keys)
    expect(collapsed.has("small")).toBe(false);
    // depth-2 nested object is collapsed
    expect(collapsed.has("deep.level1")).toBe(true);
  });

  it("returns an empty set for a scalar root", () => {
    expect(initialCollapsedPaths(5).size).toBe(0);
  });
});

describe("flattenJson", () => {
  it("emits open/close markers and leaves for an expanded tree", () => {
    const nodes = flattenJson({ a: 1, b: [2] }, new Set());
    const kinds = nodes.map((n) => n.kind);
    expect(kinds[0]).toBe("container-open"); // root object
    expect(kinds).toContain("leaf");
    expect(kinds).toContain("container-close");
    const leafA = nodes.find((n) => n.kind === "leaf" && n.label === "a");
    expect(leafA && "value" in leafA && leafA.value).toBe(1);
  });

  it("omits children + close marker for a collapsed container", () => {
    const nodes = flattenJson({ a: { b: 1 } }, new Set(["a"]));
    // 'a' opens but, being collapsed, neither its child nor its
    // close marker is emitted.
    expect(
      nodes.some((n) => n.path === "a" && n.kind === "container-open"),
    ).toBe(true);
    expect(nodes.some((n) => n.path === "a::close")).toBe(false);
    expect(nodes.some((n) => "label" in n && n.label === "b")).toBe(false);
  });

  it("indexes array children by position", () => {
    const nodes = flattenJson(["x", "y"], new Set());
    const labels = nodes
      .filter((n) => n.kind === "leaf")
      .map((n) => (n.kind === "leaf" ? n.label : undefined));
    expect(labels).toEqual(["0", "1"]);
  });
});

describe("primitiveDisplay / primitiveColor", () => {
  it("renders + colors primitives distinctly", () => {
    expect(primitiveDisplay(null)).toBe("null");
    expect(primitiveDisplay("hi")).toBe('"hi"');
    expect(primitiveDisplay(3)).toBe("3");
    expect(primitiveDisplay(true)).toBe("true");

    expect(primitiveColor(null)).toBe("fg.muted");
    expect(primitiveColor("s")).toBe("green.fg");
    expect(primitiveColor(1)).toBe("blue.fg");
    expect(primitiveColor(false)).toBe("orange.fg");
    expect(primitiveColor({})).toBe("fg");
  });
});
