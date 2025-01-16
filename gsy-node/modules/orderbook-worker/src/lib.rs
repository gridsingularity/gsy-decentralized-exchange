#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::sp_runtime::transaction_validity::{TransactionValidity, ValidTransaction};

use sp_core::crypto::KeyTypeId;

pub use crate::weights::WeightInfo;
pub use pallet::*;

pub use scale_info::prelude::vec::Vec;
pub use sp_core::offchain::Timestamp;
use sp_runtime::offchain::{http, Duration};
pub use sp_std::sync::Arc;

pub mod configuration;
use configuration::OrderBookServiceURLs;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ocw!");

pub mod crypto {
	use super::KEY_TYPE;
	use scale_info::prelude::string::String;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};

	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	// implemented for mock runtime in test
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*, require_transactional, sp_runtime::traits::Hash, transactional,
	};
	use frame_system::{
		offchain::{
			AppCrypto, CreateSignedTransaction, SendTransactionTypes, SendUnsignedTransaction,
			SignedPayload, Signer, SigningTypes,
		},
		pallet_prelude::*,
	};
	use gsy_primitives::v0::{
		Bid, InputOrder, Offer, Order, OrderReference, OrderSchema, OrderStatus,
	};
	use gsy_primitives::Trade;
	use scale_info::prelude::vec;
	use scale_info::TypeInfo;
	use sp_runtime::offchain::http::Request;

	#[pallet::config]
	pub trait Config:
		CreateSignedTransaction<Call<Self>>
		+ SendTransactionTypes<Call<Self>>
		+ frame_system::Config
		+ orderbook_registry::Config
		+ gsy_collateral::Config
	{
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		type RuntimeEvent: From<Event<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>
			+ Into<<Self as frame_system::Config>::RuntimeEvent>;

		/// A dispatchable call type. We need to define it for the Orderbook worker to
		/// reference the `send_response` function it wants to call.
		type Call: From<Call<Self>> + Into<<Self as frame_system::Config>::RuntimeCall>;

		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	// #[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn orderbook)]
	/// Temporary orders book for Orderbook workers.
	pub type OrdersForWorker<T: Config> = StorageMap<
		_,
		Twox64Concat,
		OrderReference<T::AccountId, T::Hash>,
		Order<T::AccountId>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn trades_for_worker)]
	/// Temporary trades for Orderbook workers.
	pub type TradesForWorker<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash,
		Trade<T::AccountId, T::Hash>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn user_nonce)]
	pub type UserNonce<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u32>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event generated when the Orderbook worker processes a request.
		/// The first value is the nonce of the request, the second the result.
		RequestClosed(u8, u8),
		/// Order has been deleted from the book.
		OrderRemoved(T::AccountId, T::Hash),
		/// New Order added to the orders book \[sender, hash\].
		NewOrderInserted(Order<T::AccountId>, T::Hash),
		NewTradeInserted(Trade<T::AccountId, T::Hash>, T::Hash),
	}

	#[pallet::error]
	pub enum Error<T> {
		OffchainUnsignedTxError,
		OffchainSignedTxError,
		OrderProcessingError,
		NoLocalAcctForSigning,
		NonceCheckOverflow,
		OrderIsNotRegistered,
		NotARootUser,
		InsufficientCollateral,
		InvalidNonce,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_block_number: BlockNumberFor<T>) {
			log::info!("Entering offchain worker...");
			Self::offchain_process();
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Insert new orders.
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The user who wants to insert the orders.
		/// `orders`: The batch of orders order.

		#[transactional]
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config >::WeightInfo::insert_orders())]
		pub fn insert_orders(
			origin: OriginFor<T>,
			orders: Vec<InputOrder<T::AccountId>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			log::info!("add orders: {:?} for the user: {:?}", orders, sender);
			let hashed_orders = Self::create_hash_vec_from_order_list(orders.clone());
			// TODO: Refactor this method to add all orders in one go.
			let _ = <orderbook_registry::Pallet<T>>::insert_orders(origin, hashed_orders);
			for order in orders {
				Self::add_order(sender.clone(), Self::input_order_to_order(order))?;
			}
			Ok(())
		}

		/// Insert new orders as a proxy account
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The user (proxy account) who wants to insert the orders.
		/// `delegator`: The user who is delegating the order.
		/// `orders`: The batch of orders order.
		#[transactional]
		#[pallet::weight(< T as Config >::WeightInfo::insert_orders_by_proxy())]
		#[pallet::call_index(1)]
		pub fn insert_orders_by_proxy(
			origin: OriginFor<T>,
			delegator: T::AccountId,
			orders: Vec<InputOrder<T::AccountId>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			log::info!(
				"add orders: {:?} for the user: {:?} - by the proxy {:?}",
				orders,
				delegator,
				sender
			);
			let hashed_orders = Self::create_hash_vec_from_order_list(orders.clone());
			let _ = <orderbook_registry::Pallet<T>>::insert_orders_by_proxy(
				origin,
				delegator.clone(),
				hashed_orders,
			);
			for order in orders {
				Self::add_order(delegator.clone(), Self::input_order_to_order(order))?;
			}
			Ok(())
		}

		/// Remove orders from the orders book.
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The user who wants to remove the orders.
		/// `orders_hash`: The batch of orders hash to remove.
		#[transactional]
		#[pallet::weight(< T as Config >::WeightInfo::remove_orders())]
		#[pallet::call_index(2)]
		pub fn remove_orders(origin: OriginFor<T>, orders_hash: Vec<T::Hash>) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			log::info!("remove orders: {:?} for the user: {:?}", orders_hash, sender);
			let _ = <orderbook_registry::Pallet<T>>::delete_orders(origin, orders_hash);
			Ok(())
		}

		/// Remove single order from the orders book if request to post order to db failed.
		/// Called by the Orderbook worker.
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The user who wants to remove the order.
		/// `order_hash`: The hash of the order to remove.
		#[pallet::weight(< T as Config >::WeightInfo::zero_weight())]
		#[pallet::call_index(3)]
		pub fn remove_order_by_order_reference(
			origin: OriginFor<T>,
			order_payload: Payload<T::Public, T::AccountId, T::Hash>,
			_signature: T::Signature,
		) -> DispatchResult {
			ensure_none(origin.clone())?;
			for payload in order_payload.order_reference {
				log::info!(
					"remove order by hash: {:?} for the user: {:?}",
					payload.hash,
					payload.user_id
				);
				let mut hash_vector = Vec::<T::Hash>::new();
				hash_vector.push(payload.hash);
				<orderbook_registry::Pallet<T>>::delete_orders(origin.clone(), hash_vector)?;
				Self::delete_order(payload)?;
			}
			Ok(())
		}

		/// Remove single order from the orders book if request to post order to db succeeded.
		/// Called by the Orderbook worker.
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The user who wants to remove the order.
		/// `order_hash`: The hash of the order to remove.
		#[pallet::weight(< T as Config >::WeightInfo::zero_weight())]
		#[pallet::call_index(4)]
		pub fn remove_local_order_by_order_reference(
			origin: OriginFor<T>,
			order_payload: Payload<T::Public, T::AccountId, T::Hash>,
			_signature: T::Signature,
		) -> DispatchResult {
			ensure_none(origin)?;
			for payload in order_payload.order_reference {
				log::info!(
					"remove local order by hash: {:?} for the user: {:?}",
					payload.hash,
					payload.user_id
				);
				Self::delete_order(payload)?;
			}
			Ok(())
		}

		/// Remove orders from the orders book as a proxy account
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The user (proxy account) who wants to remove the orders.
		/// `delegator`: The user who is delegating the order removal.
		/// `orders_hash`: The batch of orders hash to remove.
		#[transactional]
		#[pallet::weight(< T as Config >::WeightInfo::remove_orders_by_proxy())]
		#[pallet::call_index(5)]
		pub fn remove_orders_by_proxy(
			origin: OriginFor<T>,
			delegator: T::AccountId,
			orders_hash: Vec<T::Hash>,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			log::info!(
				"remove orders: {:?} for the user: {:?} - by the proxy {:?}",
				orders_hash,
				delegator,
				sender
			);

			let _ = <orderbook_registry::Pallet<T>>::delete_orders_by_proxy(
				origin,
				delegator.clone(),
				orders_hash.clone(),
			);
			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validate unsigned call to this module.
		///
		/// By default unsigned transactions are disallowed, but implementing the validator
		/// here we make sure that some particular calls (the ones produced by offchain worker)
		/// are being whitelisted and marked as valid.

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("gsy-node")
					.priority(TransactionPriority::max_value())
					.and_provides([&provide])
					.longevity(3)
					.propagate(true)
					.build()
			};

			match call {
				Call::remove_local_order_by_order_reference {
					order_payload: ref payload,
					ref signature,
				} => {
					if !SignedPayload::<T>::verify::<T::AuthorityId>(payload, signature.clone()) {
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(b"remove_local_order_by_order_reference".to_vec())
				},

				Call::remove_order_by_order_reference {
					order_payload: ref payload,
					ref signature,
				} => {
					if !SignedPayload::<T>::verify::<T::AuthorityId>(payload, signature.clone()) {
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(b"remove_order_by_order_reference".to_vec())
				},

				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	impl<T: SigningTypes> SignedPayload<T> for Payload<T::Public, T::AccountId, T::Hash> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
	pub struct Payload<Public, AccountId, Hash> {
		order_reference: Vec<OrderReference<AccountId, Hash>>,
		public: Public,
	}

	impl<T: Config> Pallet<T> {
		pub fn input_order_to_order(order: InputOrder<T::AccountId>) -> Order<T::AccountId> {
			match &order {
				InputOrder::Bid(input_order) => Order::Bid {
					0: Bid {
						buyer: input_order.buyer.clone(),
						nonce: Self::get_and_increment_user_nonce(input_order.buyer.clone()),
						bid_component: input_order.bid_component.clone(),
					},
				},
				InputOrder::Offer(input_order) => Order::Offer {
					0: Offer {
						seller: input_order.seller.clone(),
						nonce: Self::get_and_increment_user_nonce(input_order.seller.clone()),
						offer_component: input_order.offer_component.clone(),
					},
				},
			}
		}
		/// The main entry point for the offchain worker.
		fn offchain_process() {
			log::info!("Started offchain process...");
			// Iterate through the locally stored orders and react to them.
			// When the worker sees a new order, it responds by making
			// an HTTP request to the DB and send a signed transaction back.
			// After the order is stored in the DB, it is removed from storage.
			// The transaction will be sent in the following block.

			let mut orders = Vec::<Order<T::AccountId>>::new();

			let mut trades = Vec::<Trade<T::AccountId, T::Hash>>::new();

			for (order_ref, order) in <OrdersForWorker<T>>::iter() {
				match &order {
					_order_in_book => {
						log::info!(
							"Offchain process: reference: {:?}, order: {:?}",
							&order_ref,
							&order
						);
						orders.push(order);
					},
				}
			}
			if !orders.is_empty() {
				let orders_schema: Vec<OrderSchema<T::AccountId, T::Hash>> = orders
					.clone()
					.into_iter()
					.map(|order| OrderSchema {
						_id: T::Hashing::hash_of(&order),
						status: OrderStatus::Open,
						order,
					})
					.collect();
				let bytes = orders_schema.encode();
				let bytes_to_json: Vec<u8> = serde_json::to_vec(&bytes).unwrap();
				let post_order_on_db =
					Self::send_order_to_orderbook_service(&bytes_to_json).unwrap();

				if post_order_on_db == 200 {
					Self::remove_processed_orders_succeeded(orders)
						.expect("Error while removing processed orders");
				} else if post_order_on_db != 200 {
					log::warn!("Unexpected status code: {}", post_order_on_db);
					Self::remove_processed_orders_failed(orders)
						.expect("Error while removing processed orders");
				}
			}

			// TODO: Trades transmission process starts here

			for (trade_hash, trade) in <TradesForWorker<T>>::iter() {
				match &trade {
					_trade_in_book => {
						log::info!(
							"Offchain process: reference: {:?}, order: {:?}",
							&trade_hash,
							&trade
						);
						trades.push(trade);
					},
				}
			}

			if !trades.is_empty() {
				// let trade_schema: Vec<Trade<T::AccountId, T::Hash>> = trades
				// 	.clone()
				// 	.into_iter()
				// 	.map(|trade| Trade {
				// 		seller: trade.seller,
				// 		buyer: trade.buyer,
				// 		market_id: trade.market_id,
				// 		trade_uuid: trade.trade_uuid,
				// 		creation_time: trade.creation_time,
				// 		time_slot: trade.time_slot,
				// 		offer: trade.offer,
				// 		offer_hash: trade.offer_hash,
				// 		bid: trade.bid,
				// 		bid_hash: trade.bid_hash,
				// 		residual_offer
				//
				// 	})
				// 	.collect();
				let bytes = trades.encode();
				let bytes_to_json: Vec<u8> = serde_json::to_vec(&bytes).unwrap();
				let post_trades_status_code =
					Self::send_trade_to_orderbook_service(&bytes_to_json).unwrap();

				if post_trades_status_code != 200 {
					log::warn!(
						"Offchain worker failed to send trades to the orderbook service, HTTP \
						response code {}", post_trades_status_code)
				}
			}
		}

		pub fn send_trade_to_orderbook_service(request_body: &[u8]) -> Result<u16, http::Error> {
			// deadline sets the offchain worker execution time minimal as possible. So we hard
			// code the duration to 2s to complete the external call to the database to post the
			// orders.
			let orderbook_service_urls = OrderBookServiceURLs::default();
			let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));
			let request = Request::post(&orderbook_service_urls.trades_url, vec![&request_body]);
			let pending = request
				.deadline(deadline)
				.add_header("Content-Type", "application/json")
				.send()
				.map_err(|_| http::Error::DeadlineReached)?;
			let response =
				pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;
			Ok(response.code)
		}

		pub fn send_order_to_orderbook_service(request_body: &[u8]) -> Result<u16, http::Error> {
			// deadline sets the offchain worker execution time minimal as possible. So we hard
			// code the duration to 2s to complete the external call to the database to post the
			// orders.
			let orderbook_service_url = OrderBookServiceURLs::default();
			let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));
			let request = Request::post(&orderbook_service_url.orders_url, vec![&request_body]);
			let pending = request
				.deadline(deadline)
				.add_header("Content-Type", "application/json")
				.send()
				.map_err(|_| http::Error::DeadlineReached)?;
			let response =
				pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;
			Ok(response.code)
		}

		/// Sending a signed response to the pallet.
		/// Orderbook worker calls to remove orders from storage.
		///
		/// Parameters
		/// `orders`: The orders collected by the Orderbook worker from storage.
		pub fn remove_processed_orders_failed(
			orders: Vec<Order<T::AccountId>>,
		) -> Result<(), Error<T>> {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			let mut order_reference_vec = Vec::<OrderReference<T::AccountId, T::Hash>>::new();
			for order in orders {
				let order_hash = T::Hashing::hash_of(&order);
				let order_ref = Self::get_order_owner_id(order.clone());
				let order_reference =
					OrderReference { user_id: order_ref.clone(), hash: order_hash.clone() };
				order_reference_vec.push(order_reference)
			}

			if let Some((_, res)) = signer.send_unsigned_transaction(
				move |account| Payload {
					order_reference: order_reference_vec.clone(),
					public: account.public.clone(),
				},
				move |payload, signature| Call::remove_order_by_order_reference {
					order_payload: payload,
					signature,
				},
			) {
				match res {
					Ok(_) => log::info!("Unsigned transaction - remove_processed_orders_succeeded"),
					Err(()) => log::error!("{:?}", <Error<T>>::OffchainSignedTxError),
				};
			};
			Ok(())
		}

		/// Sending a signed response to the pallet.
		/// Orderbook worker calls to remove orders from storage.
		///
		/// Parameters
		/// `orders`: The orders collected by the Orderbook worker from storage.
		pub fn remove_processed_orders_succeeded(
			orders: Vec<Order<T::AccountId>>,
		) -> Result<(), Error<T>> {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			let mut order_reference_vec = Vec::<OrderReference<T::AccountId, T::Hash>>::new();
			for order in orders {
				let order_hash = T::Hashing::hash_of(&order);
				let order_ref = Self::get_order_owner_id(order.clone());
				let order_reference =
					OrderReference { user_id: order_ref.clone(), hash: order_hash.clone() };
				order_reference_vec.push(order_reference)
			}

			if let Some((_, res)) = signer.send_unsigned_transaction(
				move |account| Payload {
					order_reference: order_reference_vec.clone(),
					public: account.public.clone(),
				},
				move |payload, signature| Call::remove_local_order_by_order_reference {
					order_payload: payload,
					signature,
				},
			) {
				match res {
					Ok(_) => log::info!("Unsigned transaction - remove_processed_orders_succeeded"),
					Err(()) => log::error!("{:?}", <Error<T>>::OffchainSignedTxError),
				};
			};
			Ok(())
		}

		/// Insert a new order into the orders book.
		///
		/// Parameters
		/// `sender`: The sender of the order.
		/// `order`: The order to be inserted.
		#[require_transactional]
		pub fn add_order(sender: T::AccountId, order: Order<T::AccountId>) -> DispatchResult {
			ensure!(
				<gsy_collateral::Pallet<T>>::verify_collateral_amount(
					Self::get_order_amount(order.clone()),
					&sender
				),
				<Error<T>>::InsufficientCollateral
			);
			let order_hash = T::Hashing::hash_of(&order);
			let order_reference =
				OrderReference { user_id: sender.clone(), hash: order_hash.clone() };
			<OrdersForWorker<T>>::insert(order_reference, order.clone());
			Self::deposit_event(Event::NewOrderInserted(order, order_hash));
			Ok(())
		}

		/// Insert a new trade object into the Trades storage for offchain worker to relay them to
		/// orderbook service.
		///
		/// Parameters
		/// `sender`: The sender of the trade.
		/// `trade`: The order to be inserted.
		#[require_transactional]
		pub fn add_trade(_sender: T::AccountId, trade: Trade<T::AccountId, T::Hash>) -> DispatchResult {
			let trade_hash = T::Hashing::hash_of(&trade);
			<TradesForWorker<T>>::insert(trade_hash, trade.clone());
			Self::deposit_event(Event::NewTradeInserted(trade, trade_hash));
			Ok(())
		}

		/// Get nonce for the order.
		///
		/// Parameters
		/// `sender`: The sender of the order.
		/// Returns
		/// `u32`: The nonce for the order.
		pub fn get_and_increment_user_nonce(sender: T::AccountId) -> u32 {
			let user_nonce = <UserNonce<T>>::get(sender.clone()).unwrap_or(0u32);
			let nonce = user_nonce.checked_add(1u32).ok_or(<Error<T>>::NonceCheckOverflow).unwrap();
			<UserNonce<T>>::insert(sender.clone(), nonce);
			user_nonce
		}

		/// Remove a order from the orders book.
		///
		/// Parameters
		/// `order_reference`: The order reference.
		pub fn delete_order(
			order_reference: OrderReference<T::AccountId, T::Hash>,
		) -> DispatchResult {
			ensure!(Self::is_order_registered(&order_reference), <Error<T>>::OrderIsNotRegistered);
			<OrdersForWorker<T>>::remove(order_reference.clone());
			Self::deposit_event(Event::OrderRemoved(order_reference.user_id, order_reference.hash));
			Ok(())
		}

		/// Helper function to check if a given order has already been registered.
		///
		/// Parameters
		/// `order_ref`: The order reference.
		pub fn is_order_registered(order_ref: &OrderReference<T::AccountId, T::Hash>) -> bool {
			<OrdersForWorker<T>>::contains_key(order_ref)
		}

		/// Helper function to get the user_id of the order
		///
		/// Parameters
		/// 'order': The order reference.
		pub fn get_order_owner_id(order: Order<T::AccountId>) -> T::AccountId {
			match order {
				Order::Offer(offer) => offer.seller.clone(),
				Order::Bid(bid) => bid.buyer.clone(),
			}
		}

		/// Helper function to get the order_amount of the order
		///
		/// Parameters
		/// 'order': The order
		pub fn get_order_amount(order: Order<T::AccountId>) -> u64 {
			match order {
				Order::Offer(offer) => offer
					.offer_component
					.energy
					.clone()
					.checked_mul(offer.offer_component.energy_rate.clone())
					.unwrap(),
				Order::Bid(bid) => bid
					.bid_component
					.energy
					.clone()
					.checked_mul(bid.bid_component.energy_rate.clone())
					.unwrap(),
			}
		}

		pub fn create_hash_vec_from_order_list(
			orders: Vec<InputOrder<T::AccountId>>,
		) -> Vec<T::Hash> {
			return orders
				.clone()
				.into_iter()
				.map(|order| Self::input_order_to_order(order))
				.map(|order| T::Hashing::hash_of(&order))
				.collect();
		}
	}
}
