//! GSy Node CLI library.
#![warn(missing_docs)]
#![allow(clippy::result_large_err)]

mod chain_spec;
#[macro_use]
mod service;
mod benchmarking;
mod cli;
mod command;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}
