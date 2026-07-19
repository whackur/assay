use std::str::FromStr;

use assay_domain::ContentHash;
use sha2::{Digest, Sha256};

pub(crate) fn length_prefixed(components: &[String]) -> String {
    let mut material = String::new();
    for component in components {
        use std::fmt::Write as _;
        write!(&mut material, "{}:{}|", component.len(), component)
            .expect("writing to a String cannot fail");
    }
    material
}

pub(crate) fn sha256_content_hash(bytes: &[u8]) -> ContentHash {
    let digest = Sha256::digest(bytes);
    ContentHash::from_str(&format!("sha256:{}", hex::encode(digest)))
        .expect("SHA-256 always produces a valid domain content hash")
}
