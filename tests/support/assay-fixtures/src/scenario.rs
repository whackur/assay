//! The nine synthetic repository histories required by the Assay foundation.

use crate::spec::{CommitSpec, FileSpec};

/// The nine synthetic histories required by the Assay foundation milestone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryScenario {
    /// TypeScript production, test, README, license, and CI files.
    TypeScriptProject,
    /// Python production, test, package metadata, and documentation files.
    PythonProject,
    /// A second commit that changes only a dependency manifest and lockfile.
    DependencyOnlyChange,
    /// Generated and vendored paths declared through `.gitattributes`.
    GeneratedAndVendoredOverrides,
    /// A second commit that changes only ASCII formatting.
    FormattingOnlyChange,
    /// An unchanged file renamed and moved by a second commit.
    RenameAndMove,
    /// Supported TypeScript and Python mixed with unsupported Rust and C.
    SupportedAndUnsupportedLanguages,
    /// A repository that intentionally has no README or license.
    MissingReadmeAndLicense,
    /// Spaces and Unicode in both the repository and tracked file paths.
    SpaceAndUnicodePaths,
}

impl RepositoryScenario {
    /// All required scenarios in their stable declaration order.
    pub const ALL: [Self; 9] = [
        Self::TypeScriptProject,
        Self::PythonProject,
        Self::DependencyOnlyChange,
        Self::GeneratedAndVendoredOverrides,
        Self::FormattingOnlyChange,
        Self::RenameAndMove,
        Self::SupportedAndUnsupportedLanguages,
        Self::MissingReadmeAndLicense,
        Self::SpaceAndUnicodePaths,
    ];

    pub(crate) fn repository_name(self) -> &'static str {
        match self {
            Self::SpaceAndUnicodePaths => "fixture repository café",
            _ => "fixture-repository",
        }
    }

    pub(crate) fn commits(self) -> Vec<CommitSpec> {
        match self {
            Self::TypeScriptProject => vec![CommitSpec::new(
                "Add TypeScript project evidence",
                &[
                    FileSpec::new(
                        ".github/workflows/ci.yml",
                        b"name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n",
                    ),
                    FileSpec::new(
                        "LICENSE",
                        b"MIT License\n\nCopyright (c) 2001 Assay Fixture\n",
                    ),
                    FileSpec::new(
                        "README.md",
                        b"# TypeScript Fixture\n\nSynthetic repository evidence.\n",
                    ),
                    FileSpec::new(
                        "src/add.ts",
                        b"export function add(left: number, right: number): number {\n  return left + right;\n}\n",
                    ),
                    FileSpec::new(
                        "tests/add.test.ts",
                        b"import { add } from \"../src/add\";\n\nvoid add(1, 2);\n",
                    ),
                ],
                &[],
            )],
            Self::PythonProject => vec![CommitSpec::new(
                "Add Python project evidence",
                &[
                    FileSpec::new(
                        "docs/usage.md",
                        b"# Usage\n\nStatic documentation fixture.\n",
                    ),
                    FileSpec::new(
                        "pyproject.toml",
                        b"[project]\nname = \"assay-fixture\"\nversion = \"0.1.0\"\n",
                    ),
                    FileSpec::new(
                        "src/assay_fixture/__init__.py",
                        b"def add(left: int, right: int) -> int:\n    return left + right\n",
                    ),
                    FileSpec::new(
                        "tests/test_add.py",
                        b"from assay_fixture import add\n\n\ndef test_add() -> None:\n    assert add(1, 2) == 3\n",
                    ),
                ],
                &[],
            )],
            Self::DependencyOnlyChange => vec![
                CommitSpec::new(
                    "Add dependency fixture",
                    &[
                        FileSpec::new(
                            "package-lock.json",
                            b"{\n  \"lockfileVersion\": 3,\n  \"packages\": {\"\": {\"dependencies\": {\"left-pad\": \"1.2.0\"}}}\n}\n",
                        ),
                        FileSpec::new(
                            "package.json",
                            b"{\n  \"name\": \"dependency-fixture\",\n  \"dependencies\": {\"left-pad\": \"1.2.0\"}\n}\n",
                        ),
                        FileSpec::new("src/index.ts", b"export const value = 1;\n"),
                    ],
                    &[],
                ),
                CommitSpec::new(
                    "Update dependencies only",
                    &[
                        FileSpec::new(
                            "package-lock.json",
                            b"{\n  \"lockfileVersion\": 3,\n  \"packages\": {\"\": {\"dependencies\": {\"left-pad\": \"1.3.0\"}}}\n}\n",
                        ),
                        FileSpec::new(
                            "package.json",
                            b"{\n  \"name\": \"dependency-fixture\",\n  \"dependencies\": {\"left-pad\": \"1.3.0\"}\n}\n",
                        ),
                    ],
                    &[],
                ),
            ],
            Self::GeneratedAndVendoredOverrides => vec![CommitSpec::new(
                "Add generated and vendored overrides",
                &[
                    FileSpec::new(
                        ".gitattributes",
                        b"generated/** linguist-generated=true\nvendor/** linguist-vendored=true\n",
                    ),
                    FileSpec::new(
                        "generated/client.ts",
                        b"export const generatedClient = true;\n",
                    ),
                    FileSpec::new(
                        "src/main.ts",
                        b"export const application = true;\n",
                    ),
                    FileSpec::new("vendor/library.py", b"VENDORED_VALUE = True\n"),
                ],
                &[],
            )],
            Self::FormattingOnlyChange => vec![
                CommitSpec::new(
                    "Add compact source",
                    &[FileSpec::new(
                        "src/format.ts",
                        b"export function format(value:string):string{return value.trim();}\n",
                    )],
                    &[],
                ),
                CommitSpec::new(
                    "Format source only",
                    &[FileSpec::new(
                        "src/format.ts",
                        b"export function format(value: string): string {\n  return value.trim();\n}\n",
                    )],
                    &[],
                ),
            ],
            Self::RenameAndMove => vec![
                CommitSpec::new(
                    "Add legacy module",
                    &[FileSpec::new(
                        "src/legacy.ts",
                        b"export const stableValue = 42;\n",
                    )],
                    &[],
                ),
                CommitSpec::new(
                    "Rename and move module",
                    &[FileSpec::new(
                        "src/core/renamed.ts",
                        b"export const stableValue = 42;\n",
                    )],
                    &["src/legacy.ts"],
                ),
            ],
            Self::SupportedAndUnsupportedLanguages => vec![CommitSpec::new(
                "Add mixed-language sources",
                &[
                    FileSpec::new("native/tool.c", b"int answer(void) { return 42; }\n"),
                    FileSpec::new("src/main.rs", b"pub fn answer() -> u8 { 42 }\n"),
                    FileSpec::new(
                        "src/main.ts",
                        b"export const answer: number = 42;\n",
                    ),
                    FileSpec::new("src/tool.py", b"def answer() -> int:\n    return 42\n"),
                ],
                &[],
            )],
            Self::MissingReadmeAndLicense => vec![CommitSpec::new(
                "Add project without community files",
                &[
                    FileSpec::new(
                        "package.json",
                        b"{\n  \"name\": \"missing-community-files\"\n}\n",
                    ),
                    FileSpec::new(
                        "src/main.ts",
                        b"export const documented = false;\n",
                    ),
                ],
                &[],
            )],
            Self::SpaceAndUnicodePaths => vec![CommitSpec::new(
                "Add space and Unicode paths",
                &[
                    FileSpec::new(
                        "docs/résumé.md",
                        b"# Resume\n\nSynthetic Unicode fixture.\n",
                    ),
                    FileSpec::new(
                        "src/hello world.ts",
                        b"export const greeting = \"hello\";\n",
                    ),
                ],
                &[],
            )],
        }
    }
}
