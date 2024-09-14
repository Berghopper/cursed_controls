use std::path::PathBuf;

fn main() {
    let lib_dir = PathBuf::from("src/360-raw-gadget");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=360gadget");
    println!("cargo:rerun-if-changed=src/360-raw-gadget/lib360gadget.a");
}
