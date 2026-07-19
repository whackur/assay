//! Built-in v1 path matchers.
//!
//! Split from `rules.rs` so the individual category matchers stay separate
//! from the precedence-ordered dispatcher. Each matcher is a pure function
//! over lowercased path components and the filename; none perform I/O.

pub(crate) fn contains_component(components: &[String], candidates: &[&str]) -> bool {
    components
        .iter()
        .any(|component| candidates.contains(&component.as_str()))
}

pub(crate) fn is_coverage(components: &[String], filename: &str) -> bool {
    contains_component(components, &["coverage", ".nyc_output", "htmlcov"])
        || matches!(filename, "lcov.info" | ".coverage" | "coverage.xml")
}

pub(crate) fn is_build_output(components: &[String], filename: &str) -> bool {
    contains_component(
        components,
        &["build", "dist", "out", "target", "bin", "obj"],
    ) || matches!(filename, "bundle.js" | "bundle.css")
}

pub(crate) fn is_generated(components: &[String], filename: &str) -> bool {
    contains_component(components, &["generated", "gen", "codegen"])
        || filename.contains(".generated.")
        || filename.ends_with("_pb.js")
        || filename.ends_with("_pb.d.ts")
        || filename.ends_with("_pb2.py")
        || filename.ends_with("_pb2.pyi")
        || filename.ends_with(".pb.go")
        || filename.ends_with(".g.cs")
        || is_minified(filename)
}

pub(crate) fn is_minified(filename: &str) -> bool {
    filename.contains(".min.js") || filename.contains(".min.css")
}

pub(crate) fn is_vendored(components: &[String]) -> bool {
    contains_component(
        components,
        &[
            "vendor",
            "vendored",
            "third_party",
            "third-party",
            "node_modules",
        ],
    )
}

pub(crate) fn is_ci(components: &[String], filename: &str) -> bool {
    matches!(
        components,
        [first, second, ..] if first == ".github" && second == "workflows"
    ) || contains_component(components, &[".circleci", ".buildkite"])
        || matches!(
            filename,
            ".gitlab-ci.yml"
                | ".gitlab-ci.yaml"
                | "jenkinsfile"
                | "azure-pipelines.yml"
                | "azure-pipelines.yaml"
        )
}

pub(crate) fn is_schema_migration(components: &[String], filename: &str) -> bool {
    contains_component(components, &["migration", "migrations"]) || filename.contains(".migration.")
}

pub(crate) fn is_security_policy(components: &[String], filename: &str) -> bool {
    matches!(
        filename,
        "security.md" | "security.txt" | "dependabot.yml" | "dependabot.yaml"
    ) || contains_component(components, &["security", "codeql"])
}

pub(crate) fn is_documentation(components: &[String], filename: &str) -> bool {
    contains_component(components, &["doc", "docs", "documentation"])
        || matches!(
            filename,
            "readme"
                | "readme.md"
                | "readme.rst"
                | "readme.txt"
                | "license"
                | "license.md"
                | "license.txt"
                | "copying"
                | "changelog"
                | "changelog.md"
                | "contributing.md"
                | "code_of_conduct.md"
        )
        || matches!(extension(filename), Some("md" | "mdx" | "rst" | "adoc"))
}

pub(crate) fn is_test(components: &[String], filename: &str) -> bool {
    contains_component(
        components,
        &["test", "tests", "__tests__", "spec", "specs", "fixtures"],
    ) || filename.starts_with("test_")
        || filename.contains(".test.")
        || filename.contains(".spec.")
        || filename.ends_with("_test.py")
        || filename.ends_with("_test.go")
}

pub(crate) fn is_lockfile(filename: &str) -> bool {
    matches!(
        filename,
        "bun.lock"
            | "cargo.lock"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "poetry.lock"
            | "pdm.lock"
            | "pipfile.lock"
            | "uv.lock"
            | "composer.lock"
            | "gemfile.lock"
            | "go.sum"
    )
}

pub(crate) fn is_dependency_manifest(filename: &str) -> bool {
    matches!(
        filename,
        "cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "pipfile"
            | "poetry.toml"
            | "composer.json"
            | "gemfile"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    ) || (filename.starts_with("requirements") && filename.ends_with(".txt"))
}

pub(crate) fn is_infrastructure(components: &[String], filename: &str) -> bool {
    contains_component(
        components,
        &[
            "infra",
            "infrastructure",
            "terraform",
            "k8s",
            "kubernetes",
            "helm",
            "deploy",
            "deployment",
            "ansible",
        ],
    ) || filename == "dockerfile"
        || filename.starts_with("docker-compose.")
        || matches!(extension(filename), Some("tf" | "tfvars"))
}

pub(crate) fn is_configuration(components: &[String], filename: &str) -> bool {
    contains_component(components, &["config", "configuration", ".config"])
        || matches!(
            filename,
            ".gitattributes"
                | ".gitignore"
                | ".editorconfig"
                | ".prettierrc"
                | ".eslintrc"
                | "tsconfig.json"
                | "ruff.toml"
                | "mypy.ini"
        )
        || matches!(
            extension(filename),
            Some("toml" | "yaml" | "yml" | "json" | "ini" | "cfg" | "conf")
        )
}

pub(crate) fn is_source(filename: &str) -> bool {
    matches!(
        extension(filename),
        Some(
            "js" | "jsx"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "py"
                | "pyi"
                | "rs"
                | "c"
                | "h"
                | "cc"
                | "cpp"
                | "hpp"
                | "go"
                | "java"
                | "kt"
                | "kts"
                | "rb"
                | "php"
                | "swift"
                | "cs"
                | "scala"
                | "sh"
                | "bash"
        )
    )
}

fn extension(filename: &str) -> Option<&str> {
    filename.rsplit_once('.').map(|(_, extension)| extension)
}
