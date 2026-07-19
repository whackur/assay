#![cfg(unix)]
//! The `FoundationFixture` repository builder for the vertical slice tests.

use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt, path::PathBuf};

use super::common::{REPOSITORY_EXECUTION_SENTINELS, git_command, successful};

pub(crate) struct FoundationFixture {
    pub _temporary: tempfile::TempDir,
    pub repository: PathBuf,
    pub revision: String,
    pub tripwire: PathBuf,
    pub command_shims: PathBuf,
}

impl FoundationFixture {
    pub(crate) fn build() -> Self {
        let temporary = tempfile::tempdir().expect("fixture root");
        let repository = temporary.path().join("foundation repository");
        fs::create_dir(&repository).expect("fixture repository");
        successful(
            git_command(&repository)
                .args(["init", "--quiet", "--initial-branch=main", "--template="])
                .output()
                .expect("git init"),
            "git init",
        );
        for (key, value) in [
            ("user.name", "Assay Foundation Fixture"),
            ("user.email", "foundation-fixture@example.invalid"),
            ("commit.gpgSign", "false"),
            ("core.autocrlf", "false"),
        ] {
            successful(
                git_command(&repository)
                    .args(["config", "--local", key, value])
                    .output()
                    .expect("git config"),
                "git config",
            );
        }

        let files = BTreeMap::from([
            (
                ".gitattributes",
                b"generated/** linguist-generated=true\nvendor/** linguist-vendored=true\ndocs/untrusted.txt filter=assay diff=assay\n".as_slice(),
            ),
            (
                ".github/workflows/ci.yml",
                b"name: fixture-ci\non: [push]\njobs: {test: {runs-on: ubuntu-latest}}\n".as_slice(),
            ),
            ("LICENSE", b"MIT License\n".as_slice()),
            (
                "README.md",
                b"# Foundation Fixture\n\nStatic evidence only.\n".as_slice(),
            ),
            ("SECURITY.md", b"# Security Policy\n".as_slice()),
            ("config/app.toml", b"mode = \"fixture\"\n".as_slice()),
            ("coverage/lcov.info", b"TN:\nend_of_record\n".as_slice()),
            ("dist/bundle.js", b"const built = true;\n".as_slice()),
            ("docs/guide.md", b"# Guide\n".as_slice()),
            ("docs/untrusted.txt", super::common::SECRET_MARKER.as_bytes()),
            (
                "generated/client.pb.ts",
                b"export const generated = true;\n".as_slice(),
            ),
            ("infra/main.tf", b"terraform {}\n".as_slice()),
            ("migrations/001_init.sql", b"CREATE TABLE fixture(id INT);\n".as_slice()),
            ("native/unsupported.rs", b"pub fn unsupported() {}\n".as_slice()),
            (
                "python/import_tripwire.py",
                b"from pathlib import Path\nPath('TRIPWIRE_PYTHON_IMPORT').touch()\n".as_slice(),
            ),
            (
                "package-lock.json",
                b"{\"lockfileVersion\":3,\"packages\":{}}\n".as_slice(),
            ),
            (
                "package.json",
                b"{\"name\":\"foundation-fixture\",\"scripts\":{\"preinstall\":\": > TRIPWIRE_PREINSTALL\",\"install\":\": > TRIPWIRE_INSTALL\",\"postinstall\":\": > TRIPWIRE_POSTINSTALL\",\"build\":\": > TRIPWIRE_BUILD\",\"test\":\": > TRIPWIRE_TEST\"}}\n".as_slice(),
            ),
            (
                "src/import_tripwire.js",
                b"require('fs').writeFileSync('TRIPWIRE_JS_IMPORT', 'executed');\n".as_slice(),
            ),
            (
                "src/main.ts",
                b"export const foundation = (): string => \"private-source-body\";\n".as_slice(),
            ),
            (
                "tests/main.test.ts",
                b"import { foundation } from \"../src/main\";\nvoid foundation();\n".as_slice(),
            ),
            ("vendor/library.ts", b"export const vendored = true;\n".as_slice()),
        ]);
        for (relative, contents) in files {
            let destination = repository.join(relative);
            fs::create_dir_all(destination.parent().expect("fixture file parent"))
                .expect("fixture directory");
            fs::write(destination, contents).expect("fixture file");
        }
        successful(
            git_command(&repository)
                .args(["add", "--all"])
                .output()
                .expect("git add"),
            "git add",
        );
        successful(
            git_command(&repository)
                .env("GIT_AUTHOR_DATE", "2001-02-03T04:05:06+09:00")
                .env("GIT_COMMITTER_DATE", "2001-02-03T04:05:06+09:00")
                .args(["commit", "--quiet", "-m", "Add foundation evidence"])
                .output()
                .expect("git commit"),
            "git commit",
        );
        let revision = String::from_utf8(
            successful(
                git_command(&repository)
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .expect("git rev-parse"),
                "git rev-parse",
            )
            .stdout,
        )
        .expect("ASCII revision")
        .trim()
        .to_owned();

        let tripwire = temporary.path().join("repository-code-executed");
        let trap = repository.join(".git/assay-tripwire.sh");
        fs::write(
            &trap,
            format!("#!/bin/sh\n: > '{}'\nexit 97\n", tripwire.display()),
        )
        .expect("tripwire script");
        let mut permissions = fs::metadata(&trap)
            .expect("tripwire metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&trap, permissions).expect("tripwire permissions");
        let trap = trap.to_string_lossy().into_owned();
        let trap_command = format!("'{trap}'");
        for key in [
            "filter.assay.clean",
            "filter.assay.smudge",
            "diff.assay.textconv",
        ] {
            successful(
                git_command(&repository)
                    .args(["config", "--local", key, &trap_command])
                    .output()
                    .expect("hostile local config"),
                "hostile local config",
            );
        }
        let hook = repository.join(".git/hooks/post-checkout");
        fs::create_dir_all(hook.parent().expect("hook directory")).expect("hook directory");
        fs::write(&hook, format!("#!/bin/sh\nexec '{trap}'\n")).expect("hostile hook");
        let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(hook, permissions).expect("hook permissions");
        let command_shims = temporary.path().join("command-shims");
        fs::create_dir(&command_shims).expect("command shim directory");
        for command in [
            "npm", "node", "npx", "build", "import", "python", "python3", "pip", "pip3", "cargo",
            "rustc", "make",
        ] {
            let shim = command_shims.join(command);
            fs::write(
                &shim,
                format!("#!/bin/sh\n: > '{}'\nexit 97\n", tripwire.display()),
            )
            .expect("command shim");
            let mut permissions = fs::metadata(&shim).expect("shim metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(shim, permissions).expect("shim permissions");
        }
        let shim_probe = std::process::Command::new(command_shims.join("npm"))
            .output()
            .expect("shim self-check must execute");
        assert_eq!(shim_probe.status.code(), Some(97));
        assert!(tripwire.exists(), "command shim self-check is non-vacuous");
        fs::remove_file(&tripwire).expect("reset command shim tripwire");
        assert!(!tripwire.exists());
        for sentinel in REPOSITORY_EXECUTION_SENTINELS {
            assert!(!repository.join(sentinel).exists());
        }

        Self {
            _temporary: temporary,
            repository,
            revision,
            tripwire,
            command_shims,
        }
    }
}
