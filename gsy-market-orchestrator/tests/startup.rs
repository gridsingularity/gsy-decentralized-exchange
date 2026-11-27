use std::process::Command;

#[test]
fn missing_or_invalid_actor_hash_key_fails_before_connecting() {
    for key in [None, Some("invalid-key")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gsy-market-orchestrator"));
        command.env_remove("ACTOR_ID_HASH_KEY");
        // Reaching the connector would fail with a different error.
        command.env("EVM_NODE_URL", "not-a-url");
        if let Some(key) = key {
            command.env("ACTOR_ID_HASH_KEY", key);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("ACTOR_ID_HASH_KEY"), "{stderr}");
        if let Some(key) = key {
            assert!(!stderr.contains(key), "key material must not be logged");
        }
    }
}
