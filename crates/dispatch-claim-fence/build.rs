#![forbid(unsafe_code)]
//! Build-identity stamp, delegated to `build-stamp` so there is ONE implementation.
//!
//! A binary that cannot name its build is DELETED by the installer's identity rule
//! (`installer/src/main.rs:173`), and that rule is what removed `tick-monitor` and
//! left the fleet untended for hours. This crate declares a `[[bin]]`, so it is in
//! that rule's scope; it emitted no identity until now.

fn main() {
    build_stamp::emit();
}
