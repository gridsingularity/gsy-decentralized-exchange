use config::{Config, Environment, File, FileFormat};
use launchpad_matching_service::configuration::{DEFAULT_JSON_BODY_LIMIT, configuration_from};
use std::collections::HashMap;

const BASE_YAML: &str = r#"
application_host: "0.0.0.0"
application_port: "9876"
database_host: "localhost"
database_username: "gsy"
database_password: "gsy"
database_name: "launchpad_test"
database_url_scheme: "mongodb"
jwt_secret: "test_secret"
"#;

/// Builder with `yaml` as the file source and `env` as the environment
/// variables (instead of the process environment), in the same order as
/// `get_configuration`.
fn builder(
    yaml: &str,
    env: &[(&str, &str)],
) -> config::ConfigBuilder<config::builder::DefaultState> {
    let env: HashMap<String, String> = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Config::builder()
        .add_source(File::from_str(yaml, FileFormat::Yaml))
        .add_source(Environment::default().source(Some(env)))
}

#[test]
fn json_body_limit_defaults_to_16_mib() {
    let configuration = configuration_from(builder(BASE_YAML, &[])).unwrap();
    assert_eq!(configuration.json_body_limit, 16 * 1024 * 1024);
    assert_eq!(DEFAULT_JSON_BODY_LIMIT, 16_777_216);
    // Above the largest plan limit (10 MiB, Advanced).
    assert!(DEFAULT_JSON_BODY_LIMIT > 10 * 1024 * 1024);
}

#[test]
fn json_body_limit_from_file_and_environment() {
    let yaml = format!("{}json_body_limit: \"1048576\"\n", BASE_YAML);
    let configuration = configuration_from(builder(&yaml, &[])).unwrap();
    assert_eq!(configuration.json_body_limit, 1_048_576);

    // The environment variable overrides the file.
    let configuration =
        configuration_from(builder(&yaml, &[("JSON_BODY_LIMIT", "33554432")])).unwrap();
    assert_eq!(configuration.json_body_limit, 33_554_432);
}

#[test]
fn json_body_limit_must_be_positive() {
    let error = configuration_from(builder(BASE_YAML, &[("JSON_BODY_LIMIT", "0")]))
        .err()
        .expect("0 must be rejected");
    assert!(
        error.to_string().contains("json_body_limit"),
        "error: {}",
        error
    );

    for invalid in ["-1", "abc"] {
        assert!(
            configuration_from(builder(BASE_YAML, &[("JSON_BODY_LIMIT", invalid)])).is_err(),
            "{} must be rejected",
            invalid
        );
    }
}
