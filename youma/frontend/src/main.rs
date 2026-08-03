// Trunk uses lib.rs as the WASM entry point. This binary stub prevents
// cargo check (host target) from compiling the WASM-specific lib code.
fn main() {}
