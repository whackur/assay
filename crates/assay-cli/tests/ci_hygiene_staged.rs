#![cfg(unix)]
//! Staged index audit tests for gitlinks and malformed records.

mod ci_hygiene;

use ci_hygiene::audit::audit_staged_index;
use ci_hygiene::common::{git_command, successful};

#[test]
fn staged_index_audit_rejects_gitlinks_and_malformed_records() {
    let oid = "a".repeat(40);
    let sha256_oid = "b".repeat(64);
    let normal = format!(
        "100644 {oid} 0\tREADME.md\0\
         100755 {oid} 0\tscripts/check.sh\0\
         120000 {oid} 0\tcurrent-docs\0\
         100644 {sha256_oid} 0\tsha256-object.txt\0"
    );
    audit_staged_index(normal.as_bytes()).expect("normal blobs, executables, and symlinks");

    let gitlink = format!("160000 {oid} 0\tordinary/path\0");
    assert!(audit_staged_index(gitlink.as_bytes()).is_err());
    for malformed in [
        format!("100644 {oid} 0\tmissing-nul"),
        format!("100644 {oid} 0 no-tab\0"),
        "100644 invalid 0\tbad-object\0".to_owned(),
        format!("100644 {oid} 1\tunmerged\0"),
        format!("100664 {oid} 0\tbad-mode\0"),
        format!("100644 {oid} 0\tduplicate\0100644 {oid} 0\tduplicate\0"),
        format!("100644 {oid} 0\tfirst\0\0100644 {oid} 0\tsecond\0"),
    ] {
        assert!(
            audit_staged_index(malformed.as_bytes()).is_err(),
            "malformed stage record was accepted: {malformed:?}"
        );
    }
}

#[test]
fn staged_index_audit_rejects_a_real_gitlink_entry() {
    let temporary = tempfile::tempdir().expect("temporary repository");
    successful(
        git_command(temporary.path())
            .args(["init", "--quiet"])
            .output()
            .expect("git init"),
        "git init",
    );
    successful(
        git_command(temporary.path())
            .args([
                "update-index",
                "--add",
                "--cacheinfo",
                "160000,1111111111111111111111111111111111111111,nested/repository",
            ])
            .output()
            .expect("git update-index"),
        "git update-index",
    );
    let staged = successful(
        git_command(temporary.path())
            .args(["ls-files", "--stage", "-z"])
            .output()
            .expect("git ls-files"),
        "git ls-files",
    );
    assert!(staged.stdout.starts_with(b"160000 "));
    assert!(audit_staged_index(&staged.stdout).is_err());
}
