fn main() {
    tonic_build::configure()
        .build_client(true)
        .compile_protos(&["src/protos/health.proto"], &["src/"])
        .unwrap();
}
