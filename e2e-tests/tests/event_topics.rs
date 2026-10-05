#[path = "../src/utils.rs"]
mod utils;

use ethers::types::H256;
use utils::indexed_bytes16_topic;

#[test]
fn preserves_bytes16_order_and_right_pads_with_zeros() {
    let id = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let topic = indexed_bytes16_topic(id);

    assert_eq!(&topic.as_bytes()[..16], &id);
    assert_eq!(&topic.as_bytes()[16..], &[0u8; 16]);
}

#[test]
fn encodes_zero_id_as_zero_topic() {
    assert_eq!(indexed_bytes16_topic([0; 16]), H256::zero());
}
