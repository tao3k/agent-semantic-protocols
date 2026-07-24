use super::StructuralSelectorCodecError;

pub fn encode_structural_selector_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte & 0x0f));
        }
    }
    encoded
}

pub fn decode_structural_selector_component(
    value: &str,
) -> Result<String, StructuralSelectorCodecError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(StructuralSelectorCodecError::new(
                        "incomplete percent escape in structural selector component",
                    ));
                }
                let high = decode_hex(bytes[index + 1])?;
                let low = decode_hex(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') => {
                decoded.push(byte);
                index += 1;
            }
            _ => {
                return Err(StructuralSelectorCodecError::new(
                    "non-canonical unescaped byte in structural selector component",
                ));
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| {
        StructuralSelectorCodecError::new(
            "structural selector component is not valid UTF-8 after decoding",
        )
    })
}

fn hex_digit(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'A' + (value - 10)
    })
}

fn decode_hex(value: u8) -> Result<u8, StructuralSelectorCodecError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        b'a'..=b'f' => Err(StructuralSelectorCodecError::new(
            "structural selector percent escapes must use uppercase hex",
        )),
        _ => Err(StructuralSelectorCodecError::new(
            "invalid percent escape in structural selector component",
        )),
    }
}
