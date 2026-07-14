use subxt::utils::H256;
use blake2_rfc::blake2b::blake2b;

use gsy_offchain_primitives::utils::{h256_to_string, string_to_h256, community_id_from_uuid};

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
    
    #[test]
    fn community_id_is_deterministic() {
        let uuid = "9e1c4a2f-3f0b-4a6d-8f1e-2b7c5d9a0e11";
        assert_eq!(community_id_from_uuid(uuid), community_id_from_uuid(uuid));
        assert_eq!(community_id_from_uuid(uuid), community_id_from_uuid(&uuid.to_string()));
    }

    #[test]
    fn community_id_differs_for_different_communities() {
        assert_ne!(community_id_from_uuid("community-1"),
                   community_id_from_uuid("community-2"));
    }

    #[test]
    fn community_id_matches_blake2b_of_uuid_bytes() {
        let uuid = "community-1";
        let expected = H256(
            blake2b(32, &[], uuid.as_bytes())
                .as_bytes()
                .try_into()
                .expect("hash is 32 bytes"),
        );
        assert_eq!(community_id_from_uuid(uuid), expected);
    }
}
