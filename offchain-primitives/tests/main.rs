use subxt::utils::H256;

use gsy_offchain_primitives::utils::{h256_to_string, string_to_h256};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h256_to_string() {
        let hash = H256::zero();
        let hash_string = h256_to_string(hash);
        assert_eq!(hash_string, "0x0000000000000000000000000000000000000000000000000000000000000000");
    }
    
    #[test]
    fn test_string_to_h256() {
        // let hash = H256::zero();
        let zero_hash_string = "0x0000000000000000000000000000000000000000000000000000000000000000";
        let hash = string_to_h256(zero_hash_string.to_string());
        assert_eq!(hash, H256::zero());
    }

    #[test]
    fn test_string_to_h256_and_reverse_works_for_random_hashes() {
        let hash = H256::random();
        let hash_string = h256_to_string(hash);
        assert_eq!(hash, string_to_h256(hash_string));
    }
    
}