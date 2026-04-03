use tonic_prost_build::Config;

fn main() {
    let mut config = Config::new();
    config.boxed(".terrazzo.remote.tests.Expression.operation");
    tonic_prost_build::configure()
        .build_client(true)
        .compile_with_config(
            config,
            &["src/protos/health.proto", "src/protos/tests.proto"],
            &["src/"],
        )
        .unwrap();
}
