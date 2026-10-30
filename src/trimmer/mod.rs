//! Token and context compression for a project.
//!
//! Port of `scripts/internal/trimmer` (Go), which ported `plaesy-trim.sh`:
//!
//! - **Layer 1** ([`layer1`]): compresses a command's output, per tool.
//! - **Layer 2** ([`layer2`]): compresses prose in memory and instruction files.
//!   The LLM-quality path ([`llm`]) lets the calling assistant do the rewrite.
//! - **Report** ([`stats::report`]): cumulative savings across layers.

pub mod directory;
pub mod filters;
pub mod graph;
pub mod layer1;
pub mod layer2;
pub mod llm;
pub mod rules;
pub mod stats;
