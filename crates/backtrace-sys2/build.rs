use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let libbacktrace_src_dir = Path::new("src/libbacktrace").canonicalize().unwrap();

    // Run ./configure
    let configure_status = Command::new("./configure")
        .current_dir(&libbacktrace_src_dir)
        .status()
        .expect("libbacktrace: ./configure failed");
    if !configure_status.success() {
        panic!("libbacktrace: ./configure failed");
    }

    // Run make
    let make_status = Command::new("make")
        .current_dir(&libbacktrace_src_dir)
        .status()
        .expect("libbacktrace: make failed");
    if !make_status.success() {
        panic!("libbacktrace: make failed");
    }

    // Generate bindings
    let bindings = bindgen::Builder::default()
        .header(format!("{}/backtrace.h", libbacktrace_src_dir.display()))
        .allowlist_function("backtrace_.*")
        .generate_comments(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Failed to generate bindings");

    // Output bindings to the src directory
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("backtrace_bindings.rs"))
        .expect("Failed to write bindings");

    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-link-search=native={}",
        libbacktrace_src_dir.join(".libs").display()
    );
    println!("cargo:rustc-link-lib=static=backtrace");
}
