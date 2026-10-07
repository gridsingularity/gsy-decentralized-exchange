use ethers::types::H256;

/// Solidity encodes indexed bytes16 values by right-padding them to 32 bytes.
pub fn indexed_bytes16_topic(id: [u8; 16]) -> H256 {
    let mut topic = [0u8; 32];
    topic[..16].copy_from_slice(&id);
    H256::from(topic)
}
