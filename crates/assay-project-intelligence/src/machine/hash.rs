use sha2::{Digest, Sha256};

pub(crate) fn stable_hash(bytes: &[u8]) -> String {
    format!("sha256:{}", stable_hex(bytes))
}

pub(crate) fn stable_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    stable_hash(bytes)
}
