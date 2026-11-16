#[path = "../src/feature_selection.rs"]
mod feature_selection;

use cucumber::{
    parser::{basic::Cli, Basic},
    Parser,
};
use feature_selection::{configure_features, feature_matching_algorithm};
use primitives::MatchingAlgorithm;
use std::collections::BTreeSet;
use std::ffi::OsStr;

fn default_features() -> Vec<cucumber::gherkin::Feature> {
    let mut cli = Cli::default();
    configure_features(&mut cli, None);
    Basic::new()
        .parse("features", cli)
        .into_inner()
        .map(Result::unwrap)
        .collect()
}

fn directory(feature: &cucumber::gherkin::Feature) -> &OsStr {
    feature
        .path
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
}

#[test]
fn default_run_parses_every_algorithm_and_the_shared_policy_once() {
    let features = default_features();
    let directories = features.iter().map(directory).collect::<BTreeSet<_>>();
    assert_eq!(
        directories,
        ["matching_algorithms", "pay_as_bid", "pay_as_clear"]
            .map(OsStr::new)
            .into_iter()
            .collect()
    );
    let policies = features
        .iter()
        .filter(|feature| feature.name == "Preference settlement policy")
        .collect::<Vec<_>>();
    assert_eq!(policies.len(), 1);
    // Nine cases, each under pay_as_bid and pay_as_clear.
    assert_eq!(policies[0].scenarios.len(), 18);
}

#[test]
fn a_feature_uses_the_algorithm_of_its_directory() {
    for feature in default_features() {
        let expected = if directory(&feature) == "pay_as_clear" {
            MatchingAlgorithm::PayAsClear
        } else {
            MatchingAlgorithm::PayAsBid
        };
        assert_eq!(
            feature_matching_algorithm(feature.path.as_deref()),
            expected,
            "{:?}",
            feature.path
        );
    }
    assert_eq!(
        feature_matching_algorithm(None),
        MatchingAlgorithm::PayAsBid
    );
}

#[test]
fn explicit_feature_path_does_not_add_shared_features() {
    let path = "features/green_proofs";
    let mut cli = Cli::default();
    configure_features(&mut cli, Some(path));
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
    configure_features(&mut cli, None);
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

#[test]
fn explicit_cli_input_can_select_multiple_feature_groups() {
    let mut cli = Cli {
        features: Some(
            "features/{pay_as_bid,matching_algorithms,green_proofs}/**/*.feature"
                .parse()
                .unwrap(),
        ),
    };
    configure_features(&mut cli, None);
    let features = Basic::new()
        .parse("features", cli)
        .into_inner()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    let directories = features
        .iter()
        .map(|feature| {
            feature.path.as_ref().unwrap().parent().unwrap().file_name().unwrap()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        directories,
        ["pay_as_bid", "matching_algorithms", "green_proofs"]
            .map(OsStr::new)
            .into_iter()
            .collect()
    );
}
