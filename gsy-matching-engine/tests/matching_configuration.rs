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

fn startup_output(algorithm: &str, pricing: Option<&str>) -> (bool, String) {
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
    command.env("MATCHING_ALGORITHM", algorithm);
    command.env_remove("PAY_AS_CLEAR_PRICING");
    if let Some(pricing) = pricing {
        command.env("PAY_AS_CLEAR_PRICING", pricing);
    }
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
        let (_, output) = startup_output("pay_as_clear", configured);
        assert!(
            output.contains(&format!("Using pay-as-clear pricing: {expected}")),
            "{output}"
        );
        assert!(output.contains("Connecting to EVM node"), "{output}");
    }
}

#[test]
fn invalid_pricing_fails_before_connecting() {
    let (success, output) = startup_output("pay_as_clear", Some("unknown"));
    assert!(!success);
    assert!(output.contains("Invalid PAY_AS_CLEAR_PRICING"), "{output}");
    assert!(
        output.contains("max_offer, min_bid, or midpoint"),
        "{output}"
    );
    assert!(!output.contains("Connecting to EVM node"), "{output}");
}

#[test]
fn pay_as_bid_ignores_clear_pricing_configuration() {
    let (_, output) = startup_output("pay_as_bid", Some("unknown"));
    assert!(!output.contains("Invalid PAY_AS_CLEAR_PRICING"), "{output}");
    assert!(!output.contains("Using pay-as-clear pricing"), "{output}");
    assert!(output.contains("Connecting to EVM node"), "{output}");
}
