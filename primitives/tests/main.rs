use primitives::utils::{bytes16_to_hex, parse_uuid_or_hex_bytes16};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uuid_as_bytes16() {
        let value = parse_uuid_or_hex_bytes16("00112233-4455-6677-8899-aabbccddeeff").unwrap();

        assert_eq!(bytes16_to_hex(value), "0x00112233445566778899aabbccddeeff");
    }

    #[test]
    fn parses_bytes16_hex_without_h256_padding() {
        let hex = "0xffeeddccbbaa99887766554433221100";
        let value = parse_uuid_or_hex_bytes16(hex).unwrap();

        assert_eq!(bytes16_to_hex(value), hex);
    }
}
