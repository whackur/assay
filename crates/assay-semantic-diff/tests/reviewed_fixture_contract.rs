use std::{fs, path::PathBuf};

use assay_semantic_diff::{
    ChangeKind, Language, NativeTreeSitterEngine, SemanticDiffEngine, SemanticDiffInput,
};

#[test]
fn reviewed_language_fixtures_preserve_semantic_change_meaning() {
    for language in [Language::JavaScript, Language::TypeScript, Language::Python] {
        let fixture = fixture_for(language);
        let engine = NativeTreeSitterEngine::new();

        let format_only = analyze(&engine, language, &fixture, "format");
        assert!(format_only.parse_errors().is_empty());
        assert!(format_only.operations().is_empty());

        let modified = analyze(&engine, language, &fixture, "modified");
        assert_eq!(modified.kinds(), vec![ChangeKind::Modified]);

        let moved = analyze(&engine, language, &fixture, "moved");
        assert_eq!(moved.kinds(), vec![ChangeKind::Moved]);

        let renamed = analyze(&engine, language, &fixture, "renamed");
        assert_eq!(renamed.kinds(), vec![ChangeKind::Renamed]);
    }
}

#[test]
fn parse_errors_are_explicit_and_do_not_invent_operations() {
    let engine = NativeTreeSitterEngine::new();
    let result = engine.analyze(SemanticDiffInput::new(
        Language::TypeScript,
        "export function valid(): number { return 1; }\n".as_bytes(),
        "export function broken(: number { return 1; }\n".as_bytes(),
    ));

    assert!(!result.parse_errors().is_empty());
    assert!(result.operations().is_empty());
}

#[test]
fn engine_metadata_is_versioned_and_does_not_claim_human_value() {
    let engine = NativeTreeSitterEngine::new();
    let metadata = engine.metadata();

    assert_eq!(metadata.engine_id(), "native-tree-sitter-1");
    assert!(!metadata.parser_version().is_empty());
    assert!(!metadata.rule_version().is_empty());
    let debug = format!("{metadata:?}").to_ascii_lowercase();
    for prohibited in ["productivity", "performance score", "human value"] {
        assert!(!debug.contains(prohibited));
    }
}

struct Fixture {
    directory: PathBuf,
    extension: &'static str,
}

fn fixture_for(language: Language) -> Fixture {
    let (directory, extension) = match language {
        Language::JavaScript => ("javascript", "js"),
        Language::TypeScript => ("typescript", "ts"),
        Language::Python => ("python", "py"),
    };
    Fixture {
        directory: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(directory),
        extension,
    }
}

fn analyze(
    engine: &NativeTreeSitterEngine,
    language: Language,
    fixture: &Fixture,
    variant: &str,
) -> assay_semantic_diff::SemanticDiffResult {
    let before = fs::read(
        fixture
            .directory
            .join(format!("before.{}", fixture.extension)),
    )
    .expect("reviewed before fixture must be readable");
    let after = fs::read(
        fixture
            .directory
            .join(format!("{variant}.{}", fixture.extension)),
    )
    .expect("reviewed after fixture must be readable");
    engine.analyze(SemanticDiffInput::new(language, &before, &after))
}
