//! Block parsing, serialization, and type registry.
//!
//! Public surface for working with executable code blocks embedded in
//! markdown notes. Submodules:
//!
//! - [`parser`] — markdown → [`ParsedBlock`]
//! - [`serializer`] — [`ParsedBlock`] → fenced markdown (canonical, deterministic)
//! - [`registry`] — alias mapping for block-type → executor names
//! - [`db_export`] — pure serializers for db `Select` results
//!   (CSV / JSON / Markdown table / INSERT statements + table-name
//!   inference). Used by the TUI export menu and reusable from the
//!   desktop once it migrates off `src/lib/blocks/db-export.ts`.
//! - [`http_normalize`] — on-read normalization of legacy
//!   JSON-bodied http blocks into the canonical HTTP-message form.
//!   Applied centrally by [`crate::fs::read_note`].

pub mod db_export;
pub mod http_codegen;
pub mod http_normalize;
pub mod parser;
pub mod registry;
pub mod serializer;

pub use parser::{blocks_above, find_block_by_alias, parse_blocks, ParsedBlock};
pub use registry::BlockTypeRegistry;
pub use serializer::serialize_block;
