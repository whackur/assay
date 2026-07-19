use assay_git::{CollectionErrorKind, CollectionLimits, CollectionStage, GitCliAdapter};
use std::path::PathBuf;

#[test]
fn reports_missing_and_incompatible_git_without_executable_paths() {
    // The path must be absolute on every platform so the failure is the
    // capability probe, not the trusted-executable shape check.
    #[cfg(windows)]
    let missing_path = PathBuf::from(r"C:\definitely\missing\assay-git");
    #[cfg(not(windows))]
    let missing_path = PathBuf::from("/definitely/missing/assay-git");
    let missing = GitCliAdapter::from_trusted_executable(missing_path, CollectionLimits::default())
        .expect_err("a missing executable must fail capability probing");
    assert_eq!(missing.stage(), CollectionStage::ProbeCapabilities);
    assert_eq!(missing.kind(), CollectionErrorKind::ExecutableMissing);
    assert!(!format!("{missing:?}").contains("definitely"));
}
