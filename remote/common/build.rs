use tonic_build::Config;

const PROTOS: &[&str] = if cfg!(debug_assertions) {
    &["src/protos/health.proto", "src/protos/tests.proto"]
} else {
    &["src/protos/health.proto"]
};

fn main() {
    let mut config = Config::new();
    config.boxed(".terrazzo.remote.tests.Expression.operation");
    tonic_build::configure()
        .build_client(true)
        .compile_protos_with_config(config, PROTOS, &["src/"])
        .unwrap();
}
