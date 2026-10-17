fn main() {
    let linker = which::which("bpf-linker").expect("bpf-linker isn't installed");
    println!("cargo:rerun-if-changed={}", linker.display());
}
