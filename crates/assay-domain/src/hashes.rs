use crate::error::{SHA256_HEX_LENGTH, SHA256_PREFIX, is_lower_hex, validated_string_value};

fn validate_sha256(value: &str) -> Result<(), &'static str> {
    let Some(digest) = value.strip_prefix(SHA256_PREFIX) else {
        return Err("expected a sha256-prefixed digest");
    };
    if digest.len() != SHA256_HEX_LENGTH {
        return Err("expected a 64-character SHA-256 digest");
    }
    if !is_lower_hex(digest) {
        return Err("expected lowercase hexadecimal characters");
    }
    Ok(())
}

validated_string_value!(ContentHash, "content_hash", validate_sha256);
