use subxt::utils::H256;


pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

pub fn h256_to_string(hash: H256) -> String {
    format!("{:?}", hash)
}

pub fn string_to_h256(hex_string: String) -> H256 {
    let hex_stripped = &hex_string[2..];
    let bytes = hex::decode(hex_stripped).expect("Invalid hex");
    H256::from_slice(&bytes)
}