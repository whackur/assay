use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};

#[test]
fn git_adapter_tests_can_consume_deterministic_repository_histories() {
    let fixture = RepositoryFixture::build(RepositoryScenario::RenameAndMove)
        .expect("the rename fixture must build");

    assert!(fixture.path().join(".git").is_dir());
    assert_eq!(fixture.commit_ids().len(), 2);
}
