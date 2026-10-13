#[path = "../src/feature_selection.rs"]
mod feature_selection;

use cucumber::{
    parser::{basic::Cli, Basic},
    Parser,
};
use feature_selection::configure_features;
use primitives::MatchingAlgorithm;

#[test]
fn each_algorithm_parses_its_own_features_and_the_shared_policy_once() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        let mut cli = Cli::default();
        configure_features(&mut cli, &algorithm, None);
        let features = Basic::new()
            .parse("features", cli)
            .into_inner()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert!(features.len() > 1);
        assert!(features.iter().all(|feature| {
            let directory = feature.path.as_ref().unwrap().parent().unwrap();
            let directory = directory.file_name().unwrap();
            directory == algorithm.as_str() || directory == "matching_algorithms"
        }));
        let policies = features
            .iter()
            .filter(|feature| feature.name == "Preference settlement policy")
            .collect::<Vec<_>>();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].scenarios.len(), 9);
    }
}

#[test]
fn explicit_feature_path_does_not_add_shared_features() {
    let path = "features/green_proofs";
    let mut cli = Cli::default();
    configure_features(&mut cli, &MatchingAlgorithm::PayAsBid, Some(path));
    assert!(cli.features.is_none());
    let features = Basic::new()
        .parse(path, cli)
        .into_inner()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(features.len(), 1);
    assert_eq!(
        features[0].path.as_ref().unwrap().file_name().unwrap(),
        "guarantees_of_origin.feature"
    );
}

#[test]
fn explicit_cli_input_is_preserved() {
    let mut cli = Cli {
        features: Some("features/pay_as_bid/trade_execution.feature".parse().unwrap()),
    };
    configure_features(&mut cli, &MatchingAlgorithm::PayAsClear, None);
    let features = Basic::new()
        .parse("features", cli)
        .into_inner()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(features.len(), 1);
    assert_eq!(
        features[0].path.as_ref().unwrap().file_name().unwrap(),
        "trade_execution.feature"
    );
}
