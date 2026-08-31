fn main() {
    println!("cargo:rerun-if-env-changed=OMP_BUILD_ID");
}
