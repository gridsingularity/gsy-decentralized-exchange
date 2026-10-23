//! `TRADER_SIGNER_SURI`: the account the community client signs and owns its orders with.

use gsy_community_client::node_connector::signer::trader_keypair;
use subxt_signer::sr25519::dev;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dev_uri_is_alice() {
        let keypair = trader_keypair("//Alice").expect("//Alice parses");
        assert_eq!(keypair.public_key().0, dev::alice().public_key().0);
    }

    #[test]
    fn another_dev_account_gives_that_account() {
        let keypair = trader_keypair("//Charlie").expect("//Charlie parses");
        assert_eq!(keypair.public_key().0, dev::charlie().public_key().0);
        assert_ne!(keypair.public_key().0, dev::alice().public_key().0);
    }

    #[test]
    fn an_invalid_uri_is_an_error_naming_the_variable() {
        let error = trader_keypair("not a // valid uri ///")
            .err()
            .expect("rejected");
        assert!(error.contains("TRADER_SIGNER_SURI"), "{error}");
    }
}
