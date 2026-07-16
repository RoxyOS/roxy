fn main() {
    let linker_script = format!("{}/../linker-x86_64.ld", env!("CARGO_MANIFEST_DIR"));

    println!("cargo::rerun-if-changed={linker_script}");
    println!("cargo::rustc-link-arg-bin=roxy-kernel=-no-pie");
    println!("cargo::rustc-link-arg-bin=roxy-kernel=-T{linker_script}");
}
