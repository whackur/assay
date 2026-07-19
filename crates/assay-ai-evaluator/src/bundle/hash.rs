use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::descriptor::EvidenceDescriptor;
use super::scope::{EvidenceScope, ExternalTransmission, privacy_code, transmission_code};

const EVIDENCE_BUNDLE_DOMAIN: &[u8] = b"assay.ai-evaluator.evidence-bundle.v1";

pub(crate) fn bundle_hash(
    scope: EvidenceScope,
    transmission: ExternalTransmission,
    items: &[EvidenceDescriptor],
) -> String {
    let mut hash = Sha256::new();
    update_length_prefixed(&mut hash, EVIDENCE_BUNDLE_DOMAIN);
    update_length_prefixed(&mut hash, privacy_code(scope));
    update_length_prefixed(&mut hash, transmission_code(transmission));
    for item in items {
        update_length_prefixed(&mut hash, item.id().as_str().as_bytes());
        update_length_prefixed(&mut hash, item.kind().code().as_bytes());
        update_length_prefixed(&mut hash, item.statement().as_bytes());
    }
    format!("sha256:{}", hex::encode(hash.finalize()))
}

fn update_length_prefixed(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

pub(crate) fn id_set(bundle: &super::EvidenceBundle) -> BTreeSet<&str> {
    bundle.items.iter().map(|item| item.id().as_str()).collect()
}
