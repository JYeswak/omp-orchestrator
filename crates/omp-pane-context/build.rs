#![forbid(unsafe_code)]
//! Build-identity stamp, delegated to `build-stamp` so there is ONE implementation.

fn main() {
    build_stamp::emit();
}
