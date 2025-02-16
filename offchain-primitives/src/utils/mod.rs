use subxt::utils::H256;
use base64;

pub fn h256_to_base64(hash: H256) -> String {
    base64::encode(hash)  // Encode the H256 into a base64 string
}

pub fn base64_to_h256(base64_str: &str) -> Result<H256, base64::DecodeError> {
    let bytes = base64::decode(base64_str)?;  // Decode the base64 string to a byte vector
    let mut hash = H256::default();          // Create a default H256 (zeroed out)
    hash.assign_from_slice(&bytes);            // Copy the bytes into the H256
    Ok(hash)
}