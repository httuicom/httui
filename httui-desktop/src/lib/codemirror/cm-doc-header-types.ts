// Shared type for the DocHeader frontmatter range.
//
// Lives in its own module so `cm-doc-header.tsx` (extension factory)
// and `cm-doc-header-state.ts` (registry + StateField wiring) can both
// reference it without importing each other.

export interface FrontmatterRange {
  /** Inclusive start offset (always 0 — frontmatter must be at top). */
  from: number;
  /** Exclusive end offset, including the trailing newline of the closing
   * fence. Use this directly as the upper bound of `Decoration.replace`. */
  to: number;
}
