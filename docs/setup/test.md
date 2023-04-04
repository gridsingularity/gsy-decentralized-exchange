## Test

You can verify the logic in your runtime by constructing a mock runtime environment. The configuration type Test is defined as a Rust enum with implementations for each of the pallet configuration traits that are used in the mock runtime.

```rust
// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		OrderbookRegistry: orderbook_registry::{Pallet, Call, Storage, Event<T>},
	}
);

impl frame_system::Config for Test {
 // -- snip --
 type MaxConsumers = frame_support::traits::ConstU32<16>;
}
```

Use Rust's native `cargo` command to build and execute the tests on the `gsy-node` runtime:

```sh
cd gsy-node
cargo test
```

For more information about using the Rust cargo test command and testing framework, run the following command:

```sh
cargo help test
```