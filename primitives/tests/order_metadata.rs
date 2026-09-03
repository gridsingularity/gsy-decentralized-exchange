use primitives::db_api_schema::orders::{
    order_metadata_from_contract, order_metadata_to_contract, ContractOrderMetadata, DbAttributes,
    DbRequirements, EnergyType,
};
use primitives::utils::{parse_or_hash_bytes16, NODE_FLOAT_SCALING_FACTOR};

const PREFERRED_PARTNER: &str = "0x00112233445566778899aabbccddeeff";
const TRADING_PARTNER: &str = "0xffeeddccbbaa99887766554433221100";

#[test]
fn converts_complete_order_metadata_to_and_from_contract_values() {
    let requirements = DbRequirements {
        trading_partner_id: Some(PREFERRED_PARTNER.to_string()),
        energy_type: Some(EnergyType::Green),
        preferred_energy_rate: Some(12.3456),
    };
    let attributes = DbAttributes {
        trading_partner_id: Some(TRADING_PARTNER.to_string()),
        energy_type: EnergyType::Pv,
    };

    let encoded = order_metadata_to_contract(Some(&requirements), Some(&attributes));

    assert_eq!(encoded.energy_source_preference, 1);
    assert_eq!(encoded.energy_type, 2);
    assert_eq!(
        encoded.preferred_trading_partner,
        parse_or_hash_bytes16(PREFERRED_PARTNER)
    );
    assert_eq!(
        encoded.preferred_energy_rate,
        (12.3456 * NODE_FLOAT_SCALING_FACTOR).round() as u64
    );
    assert_eq!(
        encoded.trading_partner,
        parse_or_hash_bytes16(TRADING_PARTNER)
    );

    assert_eq!(
        order_metadata_from_contract(encoded),
        (Some(requirements), Some(attributes))
    );
}

#[test]
fn converts_absent_order_metadata_to_and_from_zero_values() {
    let encoded = order_metadata_to_contract(None, None);

    assert_eq!(
        encoded,
        ContractOrderMetadata {
            energy_source_preference: 0,
            energy_type: 0,
            preferred_trading_partner: [0; 16],
            preferred_energy_rate: 0,
            trading_partner: [0; 16],
        }
    );
    assert_eq!(order_metadata_from_contract(encoded), (None, None));
}
