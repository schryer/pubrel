//! The corpus lock, which says which objects a release published. A pull
//! request can change it; looking a slug up must never panic.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use pubrel_fuzz::record::locked;

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    lock: &'a str,
    slug: &'a str,
}

fuzz_target!(|input: Input| {
    let _ = locked(Some(input.lock), input.slug);
    let _ = locked(Some(input.lock), "pkg.demo#identity");
});
