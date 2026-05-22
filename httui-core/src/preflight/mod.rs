//! Pre-flight checklist parsing + evaluation.
//!
//! Each item in the YAML frontmatter `preflight:` block-list declares
//! one of six check kinds (connection / env_var / branch / keychain /
//! file_exists / command) evaluated against vault context. The parser
//! reads the raw frontmatter region produced by `crate::frontmatter`
//! and owns the typed extraction for the `preflight:` section.

pub mod evaluator;
pub mod io_evaluator;
pub mod parser;

pub use evaluator::{evaluate_preflight, CheckResult, EvaluationContext};
pub use io_evaluator::evaluate_preflight_with_io;
pub use parser::{parse_preflight, PreflightItem};
