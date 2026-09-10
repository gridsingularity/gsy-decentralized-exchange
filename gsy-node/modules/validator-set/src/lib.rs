#![cfg_attr(not(feature = "std"), no_std)]

//! # Validator Set
//!
//! A minimal Proof-of-Authority validator-set manager for `pallet-session`.
//!
//! The root origin (i.e. `pallet-sudo`) can add or remove validators at
//! runtime. A change is recorded immediately but only takes effect at the start
//! of the *following* session, at which point `pallet-session` rotates the Aura
//! (block-authoring) and GRANDPA (finality) authority sets to match.
//!
//! ## Operational note
//!
//! Before a newly added account can author/finalize blocks it must have
//! registered its session keys on-chain via `Session::set_keys` (signed by that
//! account). The recommended order when onboarding a new validator is therefore:
//!
//! 1. Start the new node and generate/rotate its keys.
//! 2. Submit `Session::set_keys` from the new validator account.
//! 3. Submit `ValidatorSet::add_validator` via sudo.
//!
//! Genesis validators are bootstrapped through the `pallet-session` genesis
//! config and do not need step 2.

pub use pallet::*;

use sp_std::vec::Vec;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Maximum number of validators the set can hold.
		///
		/// This must not exceed the `MaxAuthorities` configured for Aura and
		/// GRANDPA, otherwise a session rotation could produce an authority set
		/// those pallets cannot store.
		#[pallet::constant]
		type MaxValidators: Get<u32>;

		/// Minimum number of validators that must remain in the set. Removing a
		/// validator that would take the set below this floor is rejected, to
		/// avoid accidentally stalling finality.
		#[pallet::constant]
		type MinValidators: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// The current validator set.
	#[pallet::storage]
	#[pallet::getter(fn validators)]
	pub type Validators<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxValidators>, ValueQuery>;

	/// Set to `true` when the validator set has changed but the new set has not
	/// yet been handed to `pallet-session` for the upcoming session.
	#[pallet::storage]
	pub type SetChanged<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		/// The validators active from genesis. These accounts must also appear
		/// in the `pallet-session` genesis `keys` so their session keys are
		/// registered.
		pub initial_validators: Vec<T::AccountId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let bounded = BoundedVec::<T::AccountId, T::MaxValidators>::try_from(
				self.initial_validators.clone(),
			)
			.expect("Number of initial validators exceeds MaxValidators; qed");
			assert!(
				bounded.len() as u32 >= T::MinValidators::get(),
				"Number of initial validators is below MinValidators"
			);
			Validators::<T>::put(bounded);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A validator was added to the set. Takes effect from the next session.
		ValidatorAdded { validator: T::AccountId },
		/// A validator was removed from the set. Takes effect from the next
		/// session.
		ValidatorRemoved { validator: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account is already a validator.
		AlreadyValidator,
		/// The account is not currently a validator.
		NotValidator,
		/// The set is already at `MaxValidators`.
		TooManyValidators,
		/// Removing this validator would drop the set below `MinValidators`.
		TooFewValidators,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add `who` to the validator set. Root-only. Applied from the next
		/// session.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1, 2))]
		pub fn add_validator(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			Validators::<T>::try_mutate(|validators| -> DispatchResult {
				ensure!(!validators.contains(&who), Error::<T>::AlreadyValidator);
				validators
					.try_push(who.clone())
					.map_err(|_| Error::<T>::TooManyValidators)?;
				Ok(())
			})?;
			SetChanged::<T>::put(true);
			Self::deposit_event(Event::ValidatorAdded { validator: who });
			Ok(())
		}

		/// Remove `who` from the validator set. Root-only. Applied from the next
		/// session.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1, 2))]
		pub fn remove_validator(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			Validators::<T>::try_mutate(|validators| -> DispatchResult {
				let pos = validators
					.iter()
					.position(|v| v == &who)
					.ok_or(Error::<T>::NotValidator)?;
				ensure!(
					(validators.len() as u32) > T::MinValidators::get(),
					Error::<T>::TooFewValidators
				);
				validators.remove(pos);
				Ok(())
			})?;
			SetChanged::<T>::put(true);
			Self::deposit_event(Event::ValidatorRemoved { validator: who });
			Ok(())
		}
	}
}

/// Bridge to `pallet-session`: hand the current validator set to the session
/// rotation whenever it has changed since the last session.
impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
	fn new_session(new_index: u32) -> Option<Vec<T::AccountId>> {
		// Session 0 is bootstrapped from the `pallet-session` genesis config, so
		// there is nothing for us to override there.
		if new_index == 0 {
			return None;
		}
		if SetChanged::<T>::get() {
			SetChanged::<T>::put(false);
			Some(Validators::<T>::get().into_inner())
		} else {
			// `None` tells `pallet-session` to keep the current validator set.
			None
		}
	}

	fn end_session(_end_index: u32) {}

	fn start_session(_start_index: u32) {}
}
