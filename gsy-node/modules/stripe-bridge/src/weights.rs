use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn set_stripe_enabled() -> Weight;
	fn queue_stripe_payment() -> Weight;
	fn queue_stripe_refund() -> Weight;
	fn request_balance_check() -> Weight;
	fn request_transfer_to_stripe() -> Weight;
	fn confirm_transfer_from_stripe() -> Weight;
	fn retry_transfer_to_stripe() -> Weight;
	fn force_revert_outbound_transfer() -> Weight;
	fn submit_payment_result() -> Weight;
	fn submit_refund_result() -> Weight;
	fn submit_balance_result() -> Weight;
	fn submit_outbound_transfer_result() -> Weight;
}

/// Placeholder weights for development; replace with benchmarked values.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeightInfo<T> {
	fn set_stripe_enabled() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn queue_stripe_payment() -> Weight {
		Weight::from_parts(20_000, 0)
	}
	fn queue_stripe_refund() -> Weight {
		Weight::from_parts(20_000, 0)
	}
	fn request_balance_check() -> Weight {
		Weight::from_parts(10_000, 0)
	}
	fn request_transfer_to_stripe() -> Weight {
		Weight::from_parts(20_000, 0)
	}
	fn confirm_transfer_from_stripe() -> Weight {
		Weight::from_parts(20_000, 0)
	}
	fn retry_transfer_to_stripe() -> Weight {
		Weight::from_parts(20_000, 0)
	}
	fn force_revert_outbound_transfer() -> Weight {
		Weight::from_parts(20_000, 0)
	}
	fn submit_payment_result() -> Weight {
		Weight::from_parts(15_000, 0)
	}
	fn submit_refund_result() -> Weight {
		Weight::from_parts(15_000, 0)
	}
	fn submit_balance_result() -> Weight {
		Weight::from_parts(15_000, 0)
	}
	fn submit_outbound_transfer_result() -> Weight {
		Weight::from_parts(15_000, 0)
	}
}
