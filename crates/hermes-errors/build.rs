fn main() {
    // This build script validates the canonical error registry.
    // No code generation happens here; the registry is a library.
    
    println!("cargo:rerun-if-changed=src/lib.rs");
}
