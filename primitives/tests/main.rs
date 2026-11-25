use primitives::utils::{bytes16_to_hex, bytes16_to_uuid_string, hex_to_bytes16, uuid_v4_to_bytes16};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uuid_as_bytes16() {
        let value = uuid_v4_to_bytes16("00112233-4455-4677-8899-aabbccddeeff").unwrap();

        assert_eq!(bytes16_to_hex(value), "0x00112233445546778899aabbccddeeff");
    }

    #[test]
    fn parses_bytes16_hex_without_h256_padding() {
        let hex = "0xffeeddccbbaa99887766554433221100";
        let value = hex_to_bytes16(hex).unwrap();

        assert_eq!(bytes16_to_hex(value), hex);
    }

    #[test]
    fn formats_bytes16_as_lowercase_uuid() {
        let value = hex_to_bytes16("0x00112233445566778899AABBCCDDEEFF").unwrap();

        assert_eq!(
            bytes16_to_uuid_string(value),
            "00112233-4455-6677-8899-aabbccddeeff"
        );
    }

    #[test]
    fn uuid_round_trips_through_bytes16() {
        let uuid = "11111111-1111-4111-8111-111111111111";
        let value = uuid_v4_to_bytes16(uuid).unwrap();

        assert_eq!(bytes16_to_uuid_string(value), uuid);
    }
}
