use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace = Path::new(&manifest_dir).parent().unwrap();
    let ebpf_dir = workspace.join("ebpf");
    let target_dir = workspace.join("target/ebpf");

    println!("cargo:rerun-if-env-changed=EBPF_NIGHTLY");
    let mut cargo = if env::var_os("EBPF_NIGHTLY").is_some() {
        let mut cargo = Command::new("cargo");
        cargo.arg("+nightly");
        cargo
    } else {
        let mut cargo = Command::new(env::var_os("CARGO").unwrap());
        cargo.env("RUSTC_BOOTSTRAP", "1");
        cargo
    };

    let status = cargo
        .current_dir(&ebpf_dir)
        .args(["build", "--release", "--target=bpfel-unknown-none"])
        .args(["-Z", "build-std=core"])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env_remove("RUSTC")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .status()
        .expect("failed to run cargo for the eBPF program");
    assert!(status.success(), "eBPF build failed");

    let object = target_dir.join("bpfel-unknown-none/release/wyre-ebpf");
    println!("cargo:rustc-env=EBPF_OBJECT={}", object.display());
    println!("cargo:rerun-if-changed={}", ebpf_dir.display());
}
