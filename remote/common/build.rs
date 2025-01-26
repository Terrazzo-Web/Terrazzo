fn main() {
    tonic_build::configure()
        .build_client(true)
        .compile_protos(&["src/protos/gateway_service.proto"], &["src/"])
        .unwrap();
}
