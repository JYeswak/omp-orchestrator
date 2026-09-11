#![forbid(unsafe_code)]
//! The build-identity stamp, delegated to `build-stamp` so there is ONE implementation.
//!
//! This file used to carry 23 lines duplicated across every stamped crate. The six copies had
//! already drifted apart; see `build-stamp`'s module docs for the measurement.

fn main() {
    build_stamp::emit();
}
