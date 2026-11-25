use clap::{CommandFactory, FromArgMatches};
use gsy_execution_engine::{
    services::execution_orchestrator::run_execution_cycle,
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
fn tolerance_defaults_to_five_percent_and_is_separate_from_penalty_rate() {
    let Commands::Web3 {
        penalty_rate,
        penalty_tolerance_percent,
        ..
    } = parse_cli(&["engine", "web3"]).unwrap().command;
    assert_eq!(penalty_rate, 0.1);
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

#[tokio::test]
async fn execution_cycle_rejects_invalid_tolerance_before_fetching_data() {
    for value in [-1.0, 100.1, f64::NAN, f64::INFINITY] {
        let error = run_execution_cycle("", "", "", "", 0, 0.1, 900, value)
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("Invalid penalty tolerance percentage"));
    }
}
