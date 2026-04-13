pub fn main() {
    tonic_prost_build::configure()
        .bytes(".terrazzo.terminal.LeaseItem.data")
        .bytes(".terrazzo.portforward.PortForwardDataRequest.data")
        .bytes(".terrazzo.portforward.PortForwardDataResponse.data")
        .compile_protos(
            &[
                "src/backend/protos/logs.proto",
                "src/backend/protos/notify.proto",
                "src/backend/protos/portforward.proto",
                "src/backend/protos/remote_fn.proto",
                "src/backend/protos/shared.proto",
                "src/backend/protos/terminal.proto",
            ],
            &["src/backend/protos/"],
        )
        .unwrap();
}
