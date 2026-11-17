use gsy_matching_engine::algorithms::PayAsClearPricing;
use std::process::Command;

#[test]
fn pricing_names_parse_and_display_consistently() {
    for (name, policy) in [
        ("max_offer", PayAsClearPricing::MaxOffer),
        ("min_bid", PayAsClearPricing::MinBid),
        ("midpoint", PayAsClearPricing::Midpoint),
    ] {
        assert_eq!(name.parse::<PayAsClearPricing>().unwrap(), policy);
        assert_eq!(policy.to_string(), name);
        assert_eq!(
            format!(" {} ", name.to_uppercase())
                .parse::<PayAsClearPricing>()
                .unwrap(),
            policy
        );
    }
    for name in ["", "unknown", "max_bid"] {
        assert!(name.parse::<PayAsClearPricing>().is_err());
    }
}

/// Starts the engine with `variables` on top of an environment without
/// `MATCHING_ALGORITHM` and `PAY_AS_CLEAR_PRICING`.
fn startup_output(variables: &[(&str, &str)]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gsy-matching-engine"));
    // An invalid node URL stops after startup without contacting external services.
    command.args([
        "--max-attempts",
        "0",
        "web3",
        "http://127.0.0.1",
        "8080",
        "invalid-url",
        "0",
    ]);
    command.env_remove("MATCHING_ALGORITHM");
    command.env_remove("PAY_AS_CLEAR_PRICING");
    command.envs(variables.iter().copied());
    let output = command.output().expect("matching engine should start");
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

#[test]
fn startup_defaults_and_selects_pricing() {
    for (configured, expected) in [
        (None, "max_offer"),
        (Some("max_offer"), "max_offer"),
        (Some("min_bid"), "min_bid"),
        (Some("midpoint"), "midpoint"),
    ] {
        let variables = configured
            .map(|pricing| ("PAY_AS_CLEAR_PRICING", pricing))
            .into_iter()
            .collect::<Vec<_>>();
        let (_, output) = startup_output(&variables);
        assert!(
            output.contains(&format!("Using pay-as-clear pricing: {expected}")),
            "{output}"
        );
        assert!(output.contains("Connecting to EVM node"), "{output}");
    }
}

#[test]
fn invalid_pricing_fails_before_connecting() {
    let (success, output) = startup_output(&[("PAY_AS_CLEAR_PRICING", "unknown")]);
    assert!(!success);
    assert!(output.contains("Invalid PAY_AS_CLEAR_PRICING"), "{output}");
    assert!(
        output.contains("max_offer, min_bid, or midpoint"),
        "{output}"
    );
    assert!(!output.contains("Connecting to EVM node"), "{output}");
}

#[test]
fn matching_algorithm_variable_is_ignored() {
    let (_, output) = startup_output(&[("MATCHING_ALGORITHM", "unknown")]);
    assert!(!output.contains("MATCHING_ALGORITHM"), "{output}");
    assert!(!output.contains("Using matching algorithm"), "{output}");
    assert!(output.contains("Connecting to EVM node"), "{output}");
}
