use cucumber::parser::basic::Cli;
use primitives::MatchingAlgorithm;

pub fn configure_features(
    parser: &mut Cli,
    algorithm: &MatchingAlgorithm,
    feature_override: Option<&str>,
) {
    if feature_override.is_none() && parser.features.is_none() {
        parser.features = Some(
            format!(
                "features/{{{},matching_algorithms}}/**/*.feature",
                algorithm.as_str()
            )
            .parse()
            .expect("Invalid matching feature glob"),
        );
    }
}
