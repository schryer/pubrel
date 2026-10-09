//! A package's unreleased.json, which a pull request edits by hand or with
//! `pubrel add`. Reading it must give rows or an error, never a panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use pubrel_fuzz::record::{changes, summary};

fuzz_target!(|text: &str| {
    let _ = changes(Some(text));
    let _ = summary(Some(text));
});
