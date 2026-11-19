use primitives::{
    actor_ids::ActorIdHasher,
    utils::{bytes16_to_uuid_string, hex_to_bytes16, uuid_v4_to_bytes16},
};

const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

#[test]
fn missing_key_fails_initialization() {
    // Use a fresh process so tests never mutate the cached key or each other's environment.
    if std::env::var_os("ACTOR_ID_KEY_TEST_CHILD").is_some() {
        assert!(primitives::actor_ids::initialize_actor_id_hashing().is_err());
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "missing_key_fails_initialization"])
        .env("ACTOR_ID_KEY_TEST_CHILD", "1")
        .env_remove("ACTOR_ID_HASH_KEY")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn keyed_blake2b_16_matches_independent_vector() {
    // Python hashlib.blake2b(b"facility-a", key=bytes(range(32)), digest_size=16).
    let hasher = ActorIdHasher::from_hex_key(KEY).unwrap();
    assert_eq!(
        hasher.derive("facility-a"),
        "0x2ac00d3252bb96aeba2e9500dd7f3d2c"
    );
    assert_eq!(hasher.derive("facility-a"), hasher.derive("facility-a"));
    assert_ne!(
        hasher.derive("facility-a"),
        ActorIdHasher::from_hex_key(&"ab".repeat(32))
            .unwrap()
            .derive("facility-a")
    );
    for id in ["LIC01", "ABCDEFAB-1234-4123-8123-ABCDEFABCDEF"] {
        assert_ne!(hasher.derive(id), hasher.derive(&id.to_lowercase()));
    }
    let hash = hasher.derive("facility-a");
    assert!(hex_to_bytes16(&hash).is_some());
    assert!(uuid_v4_to_bytes16(&hash).is_err());
}

#[test]
fn invalid_keys_fail_without_echoing_key_material() {
    for key in [
        "",
        "secret",
        &"ab".repeat(31),
        &"ab".repeat(33),
        &format!(" {KEY}"),
        &format!("0x{KEY}"),
    ] {
        let error = ActorIdHasher::from_hex_key(key)
            .err()
            .expect("invalid key accepted")
            .to_string();
        if !key.is_empty() {
            assert!(!error.contains(key));
        }
    }
}

#[test]
fn uuid_v4_encoding_is_reversible_without_hashing() {
    let id = "ABCDEFAB-1234-4123-8123-ABCDEFABCDEF";
    let bytes = uuid_v4_to_bytes16(id).unwrap();
    assert_eq!(bytes16_to_uuid_string(bytes), id.to_lowercase());
    for invalid in [
        "abcdefab-1234-1123-8123-abcdefabcdef",
        "abcdefab-1234-4123-0123-abcdefabcdef",
        "00000000-0000-0000-0000-000000000000",
        "{abcdefab-1234-4123-8123-abcdefabcdef}",
        "urn:uuid:abcdefab-1234-4123-8123-abcdefabcdef",
        "abcdefab123441238123abcdefabcdef",
        " abcdefab-1234-4123-8123-abcdefabcdef",
    ] {
        assert!(uuid_v4_to_bytes16(invalid).is_err(), "{invalid}");
    }
}
