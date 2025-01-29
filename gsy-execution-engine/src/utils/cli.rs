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

        #[clap(default_value_t = String::from("9944"))]
        node_port: String,

        #[clap(default_value_t = 30)]
        polling_interval: u64,

        #[clap(default_value_t = 900)]
        market_duration: u64,
    },
}
