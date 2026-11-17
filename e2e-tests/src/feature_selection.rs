use cucumber::parser::basic::Cli;
use primitives::MatchingAlgorithm;
use std::path::Path;

/// Features of a default run: every matching algorithm directory and the
/// features shared by all algorithms. `green_proofs` stays opt-in.
pub const DEFAULT_FEATURES: &str =
    "features/{pay_as_bid,pay_as_clear,matching_algorithms}/**/*.feature";

pub fn configure_features(parser: &mut Cli, feature_override: Option<&str>) {
    if feature_override.is_none() && parser.features.is_none() {
        parser.features = Some(
            DEFAULT_FEATURES
                .parse()
                .expect("Invalid matching feature glob"),
        );
    }
}

/// The matching algorithm of a feature's markets: `pay_as_clear` for the
/// features in `features/pay_as_clear`, `pay_as_bid` for all others.
pub fn feature_matching_algorithm(path: Option<&Path>) -> MatchingAlgorithm {
    let in_pay_as_clear_directory = path.is_some_and(|path| {
        path.components()
            .any(|component| component.as_os_str() == MatchingAlgorithm::PayAsClear.as_str())
    });
    if in_pay_as_clear_directory {
        MatchingAlgorithm::PayAsClear
    } else {
        MatchingAlgorithm::PayAsBid
    }
}
