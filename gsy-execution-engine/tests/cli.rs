use clap::{CommandFactory, FromArgMatches};
use gsy_execution_engine::{
    primitives::penalty_policy::PenaltyPolicy,
    utils::cli::{Cli, Commands},
};
use std::process::Command;

fn parse_cli(args: &[&str]) -> Result<Cli, clap::Error> {
    // Isolate CLI/default tests from environment overrides in the test runner.
    let matches = Cli::command()
        .mut_subcommand("web3", |command| {
            command.mut_arg("penalty_tolerance_percent", |arg| arg.env(None::<&str>))
        })
        .try_get_matches_from(args)?;
    Cli::from_arg_matches(&matches)
}

#[test]
fn tolerance_defaults_to_five_percent() {
    let Commands::Web3 {
        penalty_tolerance_percent,
        ..
    } = parse_cli(&["engine", "web3"]).unwrap().command;
    assert_eq!(penalty_tolerance_percent, 5.0);
}

#[test]
fn tolerance_accepts_percentages_including_zero_and_fractional_values() {
    for value in ["0", "2.5", "5", "100"] {
        let Commands::Web3 {
            penalty_tolerance_percent,
            ..
        } = parse_cli(&["engine", "web3", "--penalty-tolerance-percent", value])
            .unwrap()
            .command;
        assert_eq!(penalty_tolerance_percent, value.parse::<f64>().unwrap());
    }
}

#[test]
fn invalid_tolerance_fails_before_starting_execution_cycles() {
    for value in ["", "invalid", "-1", "100.1", "NaN", "inf", "-inf"] {
        assert!(parse_cli(&[
            "engine",
            "web3",
            &format!("--penalty-tolerance-percent={value}")
        ])
        .is_err());
        let output = Command::new(env!("CARGO_BIN_EXE_gsy-execution-engine"))
            .arg("web3")
            .env("EXECUTION_ENGINE_PENALTY_TOLERANCE_PERCENT", value)
            .output()
            .unwrap();
        let messages = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.status.success(), "{messages}");
        assert!(
            messages.contains("--penalty-tolerance-percent")
                && messages.contains("penalty tolerance must be a finite percentage"),
            "{messages}"
        );
        assert!(
            !messages.contains("Execution cycle for timeslot"),
            "{messages}"
        );
    }
}

#[test]
fn cli_tolerance_overrides_the_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_gsy-execution-engine"))
        .args(["web3", "--penalty-tolerance-percent", "2.5"])
        .env("EXECUTION_ENGINE_PENALTY_TOLERANCE_PERCENT", "invalid")
        // Stop after CLI parsing without making network requests.
        .env("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS", "invalid")
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        error.contains("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS"),
        "{error}"
    );
    assert!(!error.contains("penalty tolerance"), "{error}");
}

#[test]
fn pricing_policy_rejects_invalid_tolerance() {
    for value in [-1.0, 100.1, f64::NAN, f64::INFINITY] {
        let error = PenaltyPolicy::new(0.1, 0.2, value).unwrap_err();
        assert!(error
            .to_string()
            .contains("Invalid penalty tolerance percentage"));
    }
}

#[test]
fn rate_bounds_are_not_cli_arguments() {
    assert!(parse_cli(&[
        "engine",
        "web3",
        "http://localhost",
        "8080",
        "ws://localhost",
        "8545",
        "5",
        "900",
        "0.10"
    ])
    .is_err());
    assert!(parse_cli(&["engine", "web3", "--penalty-max-rate", "0.20"]).is_err());
}

#[test]
fn invalid_rate_bounds_fail_before_starting_cycles() {
    for (min, max, expected) in [
        ("NaN", "0.2", "Invalid minimum penalty rate"),
        ("invalid", "0.2", "EXECUTION_ENGINE_PENALTY_MIN_RATE"),
        ("0.1", "invalid", "EXECUTION_ENGINE_PENALTY_MAX_RATE"),
        ("0.1", "inf", "Maximum penalty rate"),
        ("0.1", "-0.2", "Maximum penalty rate"),
        ("0.1", "0.09", "Maximum penalty rate"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_gsy-execution-engine"))
            .arg("web3")
            .env("EXECUTION_ENGINE_PENALTY_MIN_RATE", min)
            .env("EXECUTION_ENGINE_PENALTY_MAX_RATE", max)
            .env("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS", "invalid")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    }
}

#[test]
fn omitted_maximum_accepts_a_minimum_above_the_shipped_maximum() {
    let output = Command::new(env!("CARGO_BIN_EXE_gsy-execution-engine"))
        .arg("web3")
        .env("EXECUTION_ENGINE_PENALTY_MIN_RATE", "0.30")
        .env_remove("EXECUTION_ENGINE_PENALTY_MAX_RATE")
        // Stop after validating the policy without making network requests.
        .env("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS", "invalid")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS")
    );
}
