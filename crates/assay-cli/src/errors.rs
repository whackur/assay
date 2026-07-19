use std::path::PathBuf;

use assay_git::{CollectionError, CollectionErrorKind, CollectionStage};

pub(crate) struct RunError {
    pub(crate) exit_code: i32,
    pub(crate) message: String,
}

pub(crate) enum Outcome {
    Emit {
        bytes: Vec<u8>,
        destination: PathBuf,
    },
    Served,
}

pub(crate) fn emit(bytes: Vec<u8>, destination: PathBuf) -> Outcome {
    Outcome::Emit { bytes, destination }
}

pub(crate) fn collection_or_not_found(error: CollectionError) -> RunError {
    if error.kind() == CollectionErrorKind::NonZeroExit
        && error.stage() == CollectionStage::ResolveRevision
    {
        RunError {
            exit_code: 4,
            message: "source_or_revision_not_found".into(),
        }
    } else {
        collection_error(error)
    }
}

pub(crate) fn collection_error(error: CollectionError) -> RunError {
    RunError {
        exit_code: 10,
        message: format!(
            "collection_failed stage={} kind={}",
            debug_code(error.stage()),
            debug_code(error.kind())
        ),
    }
}

fn debug_code(value: impl std::fmt::Debug) -> String {
    let input = format!("{value:?}");
    let mut output = String::new();
    for (index, character) in input.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    output
}

pub(crate) fn bundle_error() -> RunError {
    RunError {
        exit_code: 12,
        message: "schema_validation_failed invariant=project_bundle".into(),
    }
}

pub(crate) fn invalid_test_limit() -> RunError {
    RunError {
        exit_code: 2,
        message: "invalid_input field=collection_limit".into(),
    }
}

pub(crate) fn history_write_error() -> RunError {
    RunError {
        exit_code: 13,
        message: "history_write_failed".into(),
    }
}

pub(crate) fn invalid_clock() -> RunError {
    RunError {
        exit_code: 12,
        message: "schema_validation_failed field=generated_at".into(),
    }
}

pub(crate) fn history_operation_error() -> RunError {
    RunError {
        exit_code: 13,
        message: "history_operation_failed".into(),
    }
}

pub(crate) fn serve_bind_error() -> RunError {
    RunError {
        exit_code: 14,
        message: "serve_bind_failed".into(),
    }
}

pub(crate) fn serve_failed_error() -> RunError {
    RunError {
        exit_code: 14,
        message: "serve_failed".into(),
    }
}

pub(crate) fn analysis_failed(stage: &str) -> RunError {
    RunError {
        exit_code: 11,
        message: format!("analysis_failed stage={stage}"),
    }
}

pub(crate) fn history_record_invalid() -> RunError {
    RunError {
        exit_code: 13,
        message: "history_record_invalid".into(),
    }
}

pub(crate) fn schema_configuration_failed() -> RunError {
    RunError {
        exit_code: 12,
        message: "schema_configuration_failed".into(),
    }
}

pub(crate) fn schema_validation_failed() -> RunError {
    RunError {
        exit_code: 12,
        message: "schema_validation_failed".into(),
    }
}

pub(crate) fn output_serialization_failed() -> RunError {
    RunError {
        exit_code: 12,
        message: "output_serialization_failed".into(),
    }
}

pub(crate) fn invalid_github_token_env() -> RunError {
    RunError {
        exit_code: 2,
        message: "invalid_input field=github_token_env".into(),
    }
}

pub(crate) fn executable_missing() -> RunError {
    RunError {
        exit_code: 10,
        message: "collection_failed stage=configure_adapter kind=executable_missing".into(),
    }
}

pub(crate) fn source_not_found() -> RunError {
    RunError {
        exit_code: 4,
        message: "source_not_found".into(),
    }
}
