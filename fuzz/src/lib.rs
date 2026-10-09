//! pubrel's parsing modules, compiled from its own source for fuzzing.
//!
//! pubrel is a binary, with no library to link against, and giving it one
//! would make these modules a public API to keep stable. So the fuzz
//! targets compile the very files `src/main.rs` uses, by path. The module
//! names match pubrel's, so their `crate::` paths resolve the same way.
#![allow(dead_code, unused_imports)]

#[path = "../../src/manifest.rs"]
pub mod manifest;
#[path = "../../src/published.rs"]
pub mod published;
#[path = "../../src/record.rs"]
pub mod record;
#[path = "../../src/security.rs"]
pub mod security;
#[path = "../../src/version.rs"]
pub mod version;
