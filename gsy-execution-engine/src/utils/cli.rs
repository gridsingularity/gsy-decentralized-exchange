use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Web3 {
        #[clap(default_value_t = String::from("http://127.0.0.1"))]
        offchain_host: String,

        #[clap(default_value_t = String::from("8080"))]
        offchain_port: String,

        #[clap(default_value_t = String::from("ws://127.0.0.1"))]
        node_host: String,

        #[clap(default_value_t = String::from("8545"))]
        node_port: String,

        #[clap(default_value_t = 30)]
        polling_interval: u64,

        #[clap(default_value_t = 900)]
        market_duration: u64,

        /// Penalty rate (e.g., 0.10 for 10%)
        #[clap(default_value_t = 0.10)]
        penalty_rate: f64,

        /// Allowed deviation as a percentage of traded energy (0 disables tolerance).
        #[clap(
            long,
            env = "EXECUTION_ENGINE_PENALTY_TOLERANCE_PERCENT",
            default_value_t = 5.0,
            value_parser = parse_penalty_tolerance_percent
        )]
        penalty_tolerance_percent: f64,
    },
}

fn parse_penalty_tolerance_percent(value: &str) -> Result<f64, String> {
    match value.parse::<f64>() {
        Ok(value) if value.is_finite() && (0.0..=100.0).contains(&value) => Ok(value),
        _ => Err("penalty tolerance must be a finite percentage between 0 and 100".to_string()),
    }
}
