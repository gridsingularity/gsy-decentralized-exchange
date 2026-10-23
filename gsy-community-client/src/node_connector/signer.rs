use subxt_signer::{SecretUri, sr25519::Keypair};

/// The keypair the community client signs every order with, from a secret URI (`//Alice`,
/// a mnemonic, or a `0x…` seed). Orders carry this account as their buyer/seller, so it
/// must be a registered user with collateral in its vault.
pub fn trader_keypair(suri: &str) -> Result<Keypair, String> {
    let uri: SecretUri = suri
        .parse()
        .map_err(|error| format!("TRADER_SIGNER_SURI is not a valid secret URI: {error}"))?;
    Keypair::from_uri(&uri)
        .map_err(|error| format!("TRADER_SIGNER_SURI cannot be turned into a keypair: {error}"))
}
