use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok, codec::Encode};
use gsy_primitives::{
	v0::{AccountId, Order, OrderComponent},
	Bid,
};
use sp_core::offchain::{testing, OffchainWorkerExt};
use sp_runtime::{AccountId32, DispatchError::BadOrigin};

#[test]
fn orderbook_worker_sends_back_result() {
	new_test_ext().execute_with(|| {
		let (offchain, state) = testing::TestOffchainExt::new();
		let mut t = sp_io::TestExternalities::default();
		t.register_extension(OffchainWorkerExt::new(offchain));

		let test_data: Order<AccountId> = Order::Bid(Bid {
			buyer: AccountId32::new(*b"d43593c715fdd31c61141abd04a99f32"),
			nonce: 1,
			bid_component: OrderComponent {
				area_uuid: 1,
				market_uuid: 1u64,
				time_slot: 1,
				creation_time: 1,
				energy: 10,
				energy_rate: 1
			},
		});

		let bytes = test_data.encode();
		order_post_response(&mut state.write(), &bytes);
		t.execute_with(|| {
			let response_status = OrderbookWorker::send_order_to_gsy_service(&bytes).unwrap();
			assert_eq!(response_status, 200);
		});
	});
}

fn order_post_response(state: &mut testing::OffchainState, encoded_test_data: &[u8]) {
	state.expect_request(testing::PendingRequest {
		method: "POST".into(),
		headers: vec![(String::from("Content-Type"), String::from("application/json"))],
		uri: "http://127.0.0.1:8080/orders".into(),
		body: (encoded_test_data).to_vec(),
		response: Some(br#"{'result': 'b'}"#.to_vec()),
		sent: true,
		..Default::default()
	});
}
