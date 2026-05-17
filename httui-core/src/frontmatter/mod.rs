//! YAML frontmatter for `.md` runbook files.
//!
//! Hand-rolled parser tailored to the canvas §4 DocHeader schema:
//!
//! ```text
//! ---
//! title: "Payments — debug capture failures"
//! abstract: |
//!   Capture flow when …
//! tags: [payments, debug]
//! owner: alice
//! status: draft
//! preflight:
//!   - connection: payments-db
//! ---
//! ```
//!
//! The dependency-free approach is deliberate. Adding `serde_yaml`
//! would pull a deprecated lib; `serde_yml` adds a non-trivial dep
//! for a small bounded schema. The slice-1 parser handles scalar
//! string keys (`title`, `owner`, `status`) and flow-list `tags`;
//! block-scalar `abstract:` and block-list `preflight:` are kept
//! verbatim in the raw region so the round-trip survives even when
//! we don't yet typedly understand them.
//!
//! Round-trip contract: `assemble(raw_yaml, body) == original` for
//! any input with a `---` fence at offset 0. Typed-edit round-trip
//! (mutate a tag, write back) lands in a follow-up slice once the
//! YAML mutation API is fleshed out.

pub mod parser;

pub use parser::{
    assemble_with_body, parse_frontmatter, split_frontmatter, Frontmatter, FrontmatterStatus, Split,
};
