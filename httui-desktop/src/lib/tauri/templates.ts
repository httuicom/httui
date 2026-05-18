// coverage:exclude file — pure invoke() wrappers + IPC types.
//
// Tauri wrappers for `httui_core::templates`. The
// empty-state Templates card calls `listTemplates(vault)`
// to render the picker; the chosen template's `body` is copied
// verbatim into a fresh runbook.

import { invoke } from "@tauri-apps/api/core";

/** Where a template comes from. Mirrors Rust `TemplateSource`
 *  (snake_case serde). */
export type TemplateSource = "builtin" | "vault";

export interface Template {
  /** Stable id — file stem for vault, slug for built-ins. */
  id: string;
  /** Display name (frontmatter `title:` ?? id). */
  name: string;
  /** Short description (frontmatter `description:` ?? ""). */
  description: string;
  source: TemplateSource;
  /** Full markdown body, frontmatter included. Copied verbatim
   *  into the runbook the picker creates. */
  body: string;
}

/** List built-in + vault-local templates for the picker. Built-ins
 *  return an empty array until the embedded-templates content slice
 *  ships; vault-local templates come from
 *  `<vault>/.httui/templates/*.md`. */
export function listTemplates(vaultPath: string): Promise<Template[]> {
  return invoke("list_templates_cmd", { vaultPath });
}
