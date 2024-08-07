use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about)]
#[clap(propagate_version = true)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
    #[clap(short, long)]
    pub verbose: bool,
    #[clap(short, long, default_value_t = 3)]
    pub max_attempts: u8,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Web2 version
    Web2{
        #[clap(default_value_t = String::from("redis://127.0.0.1"))]
        orderbook_host: String,
        #[clap(default_value_t = String::from("6379"))]
        orderbook_port: String,
    },

    /// Web3 version
    Web3{
        #[clap(default_value_t = String::from("http://127.0.0.1"))]
        orderbook_host: String,
        #[clap(default_value_t = String::from("8080"))]
        orderbook_port: String,
        #[clap(default_value_t = String::from("ws://127.0.0.1"))]
        node_host: String,
        #[clap(default_value_t = String::from("9944"))]
        node_port: String,
    }
}