#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod weights;
pub use weights::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[allow(unexpected_cfgs)]
#[frame_support::pallet]
pub mod pallet {
	use crate::weights::WeightInfo;
	use frame_support::{pallet_prelude::*, traits::Get};
	use frame_system::{pallet_prelude::*, offchain::SubmitTransaction};
	use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// A soft cap for how many jobs the offchain worker tries to process per block.
		#[pallet::constant]
		type MaxJobsPerBlock: Get<u32>;
		/// Weights for the pallet's extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn jobs)]
	/// Jobs waiting for offchain processing: job_id -> (a, b)
	pub type Jobs<T: Config> = StorageMap<_, Blake2_128Concat, u64, (u64, u64), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn results)]
	/// Processed results: job_id -> result_bits (f64::to_bits)
	pub type Results<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		JobCreated { who: T::AccountId, job_id: u64, a: u64, b: u64 },
		ResultStored { job_id: u64, result_bits: u64 },
	}

	#[pallet::error]
	pub enum Error<T> {
		JobAlreadyExists,
		NoSuchJob,
		ResultAlreadyStored,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a job to be handled by the offchain worker.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_job())]
		pub fn create_job(origin: OriginFor<T>, job_id: u64, a: u64, b: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!Jobs::<T>::contains_key(&job_id), Error::<T>::JobAlreadyExists);
			Jobs::<T>::insert(job_id, (a, b));
			Self::deposit_event(Event::JobCreated { who, job_id, a, b });
			Ok(())
		}

		/// Unsigned submission of the computed result by the offchain worker.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::submit_result_unsigned())]
		pub fn submit_result_unsigned(origin: OriginFor<T>, job_id: u64, result_bits: u64) -> DispatchResult {
			ensure_none(origin)?;
			ensure!(Jobs::<T>::contains_key(&job_id), Error::<T>::NoSuchJob);
			ensure!(!Results::<T>::contains_key(&job_id), Error::<T>::ResultAlreadyStored);
			Results::<T>::insert(job_id, result_bits);
			// Optionally clear the job once processed
			Jobs::<T>::remove(job_id);
			Self::deposit_event(Event::ResultStored { job_id, result_bits });
			Ok(())
		}
	}

	// Offchain worker hook
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		T: frame_system::offchain::SendTransactionTypes<Call<T>>,
	{
		fn offchain_worker(_n: BlockNumberFor<T>) {
			// Try to process up to MaxJobsPerBlock jobs
			let mut processed: u32 = 0;
			let max = T::MaxJobsPerBlock::get();
			for (job_id, (a, b)) in Jobs::<T>::iter() {
				if processed >= max { break; }
				// Skip if already stored (race-protect)
				if Results::<T>::contains_key(&job_id) { continue; }
				// Floating-point computation example (allowed offchain):
				let af = a as f64;
				let bf = b as f64;
				let res = af / (bf + 1.0) + 0.5f64;
				let bits = res.to_bits();
				// Submit unsigned extrinsic with the result
				let call = Call::<T>::submit_result_unsigned { job_id, result_bits: bits };
				let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
					.map(|_| { processed += 1; })
					.map_err(|_| ());
			}
		}
	}

	// Validate incoming unsigned extrinsics from the offchain worker.
	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::submit_result_unsigned { job_id, result_bits: _ } => {
					if !Jobs::<T>::contains_key(job_id) { return InvalidTransaction::Stale.into(); }
					if Results::<T>::contains_key(job_id) { return InvalidTransaction::Stale.into(); }
					// Provide a basic tag to avoid duplicates in the pool
					ValidTransaction::with_tag_prefix("offchain-utils")
						.priority(100)
						.and_provides((*job_id,))
						.longevity(64_u64)
						.propagate(true)
						.build()
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// Simple utility function that sums two u64 values with saturating add.
		pub fn sum_u64(a: u64, b: u64) -> u64 { a.saturating_add(b) }

		// fn submit_unsigned_tx(call: Call<T>) -> Result<(), ()>
		// where
		// 	T: frame_system::offchain::SendTransactionTypes<Call<T>>,
		// {
		// 	SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| ())
		// }
	}
}

