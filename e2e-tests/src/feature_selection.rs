use cucumber::parser::basic::Cli;
use primitives::MatchingAlgorithm;
use std::path::Path;

/// Every matching algorithm runs in one default run.
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
/// features under a `pay_as_clear` directory, `pay_as_bid` for all others.
/// A scenario can choose another one with `the matching engine uses {string}`.
pub fn matching_algorithm_of(feature_path: Option<&Path>) -> MatchingAlgorithm {
    let in_pay_as_clear = feature_path.is_some_and(|path| {
        path.components()
            .any(|component| component.as_os_str() == MatchingAlgorithm::PayAsClear.as_str())
    });
    if in_pay_as_clear {
        MatchingAlgorithm::PayAsClear
    } else {
        MatchingAlgorithm::PayAsBid
    }
}
