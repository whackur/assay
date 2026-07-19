/// Returns true when `path` is a non-empty, non-absolute, traversal-free
/// repository-relative path with no empty components.
pub(crate) fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

/// Returns the directory containing a recognized project manifest, or `None`
/// when the entry is not a manifest file. `.` represents the repository root.
pub(crate) fn project_boundary(path: &str) -> Option<&str> {
    let (directory, file_name) = path.rsplit_once('/').unwrap_or((".", path));
    let is_manifest = matches!(
        file_name,
        "package.json"
            | "pyproject.toml"
            | "Cargo.toml"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    ) || file_name.ends_with(".csproj");
    is_manifest.then_some(directory)
}
