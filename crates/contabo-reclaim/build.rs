#![forbid(unsafe_code)]
//! Build-identity stamp, delegated to `build-stamp` so there is ONE implementation.
//!
//! A binary that cannot name its build is DELETED by the installer identity rule.

fn main() {
    build_stamp::emit();
}
