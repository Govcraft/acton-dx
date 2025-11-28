//! Build script for compiling Protocol Buffer definitions.
//!
//! This script uses `tonic-build` to generate Rust code from `.proto` files
//! for all Acton DX microservices.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = [
        "proto/auth.proto",
        "proto/data.proto",
        "proto/cedar.proto",
        "proto/cache.proto",
        "proto/email.proto",
        "proto/file.proto",
    ];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&proto_files, &["proto/"])?;

    // Re-run build if any proto file changes
    for proto in &proto_files {
        println!("cargo:rerun-if-changed={proto}");
    }

    Ok(())
}
