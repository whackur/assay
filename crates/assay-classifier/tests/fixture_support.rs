use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};

#[test]
fn classifier_tests_can_consume_repository_policy_fixtures() {
    let fixture = RepositoryFixture::build(RepositoryScenario::GeneratedAndVendoredOverrides)
        .expect("the attributes fixture must build");

    assert!(fixture.path().join(".gitattributes").is_file());
    assert!(fixture.path().join("generated/client.ts").is_file());
    assert!(fixture.path().join("vendor/library.py").is_file());
}
