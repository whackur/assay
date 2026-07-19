#![cfg(unix)]
//! YAML workflow audit helpers for the CI hygiene tests.

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ActiveYamlLine {
    pub indent: usize,
    pub text: String,
}

pub(crate) fn active_yaml_lines(workflow: &str) -> Result<Vec<ActiveYamlLine>, String> {
    workflow
        .lines()
        .filter_map(|line| {
            if line.contains('\t') {
                return Some(Err("workflow indentation contains a tab".into()));
            }
            let text = line.trim_start();
            if text.is_empty() || text.starts_with('#') {
                return None;
            }
            if text.contains(" #") {
                return Some(Err("inline workflow comments are ambiguous".into()));
            }
            let indent = line.len() - text.len();
            if indent % 2 != 0 {
                return Some(Err("workflow indentation must use two-space levels".into()));
            }
            Some(Ok(ActiveYamlLine {
                indent,
                text: text.to_owned(),
            }))
        })
        .collect()
}

pub(crate) fn audit_ci_workflow(workflow: &str) -> Result<(), String> {
    const EXPECTED_ACTIVE_WORKFLOW: &str = r#"name: CI
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: read
jobs:
  rust:
    name: Rust and schema contracts
    runs-on: ubuntu-24.04
    env:
      CARGO_TERM_COLOR: never
    steps:
      - name: Check out the repository
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          persist-credentials: false
      - name: Verify the Git adapter baseline
        shell: bash
        run: |
          git version --build-options
          version="$(git version | awk '{print $3}')"
          dpkg --compare-versions "$version" ge 2.47.0
      - name: Install the pinned Rust toolchain
        run: rustup toolchain install 1.97.0 --profile minimal --component rustfmt --component clippy
      - name: Restore Cargo dependency cache
        uses: actions/cache@5a3ec84eff668545956fd18022155c47e93e2684
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - name: Check formatting
        run: cargo fmt --check
      - name: Lint without warnings
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - name: Run workspace tests
        run: cargo test --workspace
      - name: Validate public schemas and goldens
        run: cargo test -p assay-cli --test schema_contracts
  windows:
    name: Rust and schema contracts (Windows)
    runs-on: windows-latest
    env:
      CARGO_TERM_COLOR: never
    steps:
      - name: Check out the repository
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          persist-credentials: false
      - name: Verify the Git adapter baseline
        shell: bash
        run: |
          git version --build-options
          version="$(git version | awk '{print $3}')"
          base="$(echo "$version" | sed 's/\.windows\..*//')"
          dpkg --compare-versions "$base" ge 2.47.0 || \
            python -c "import sys; sys.exit(0 if tuple(int(p) for p in '$base'.split('.')) >= (2,47,0) else 1)"
      - name: Install the pinned Rust toolchain
        run: rustup toolchain install 1.97.0 --profile minimal --component rustfmt --component clippy
      - name: Restore Cargo dependency cache
        uses: actions/cache@5a3ec84eff668545956fd18022155c47e93e2684
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - name: Check formatting
        run: cargo fmt --check
      - name: Lint without warnings
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - name: Run workspace tests
        run: cargo test --workspace
      - name: Re-run flaky assay-git tests single-threaded
        if: always()
        run: cargo test -p assay-git -- --test-threads=1
      - name: Validate public schemas and goldens
        run: cargo test -p assay-cli --test schema_contracts
  web:
    name: Web dashboard checks
    runs-on: ubuntu-24.04
    defaults:
      run:
        working-directory: web
    steps:
      - name: Check out the repository
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          persist-credentials: false
      - name: Set up Node.js
        uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: web/package-lock.json
      - name: Install dependencies
        run: npm ci
      - name: Type-check
        run: npx tsc --noEmit
      - name: Lint without warnings
        run: npm run lint
      - name: Run web tests
        run: npm test
      - name: Build the production bundle
        run: npm run build
"#;
    let active = active_yaml_lines(workflow)?;
    let expected = active_yaml_lines(EXPECTED_ACTIVE_WORKFLOW)?;
    if active != expected {
        return Err(format!(
            "active workflow structure differs from the read-only contract\nactual: {active:#?}\nexpected: {expected:#?}"
        ));
    }
    Ok(())
}
