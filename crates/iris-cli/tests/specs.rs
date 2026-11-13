//! Every file in `core/specs` must pass. Files in `core/specs/pending` describe behaviour that is
//! wanted and not built yet: they are run and counted, and do not fail the build.

use iris_cli::run_spec;
use std::path::Path;

fn run_dir(dir: &Path) -> (usize, Vec<String>) {
    let (mut checks, mut problems) = (0, Vec::new());
    let mut files: Vec<_> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "spec")).collect()).unwrap_or_default();
    files.sort();
    for path in files {
        let (n, failures) = run_spec(&std::fs::read_to_string(&path).unwrap());
        checks += n;
        problems.extend(failures.into_iter().map(|f| format!("{}:{} [{}] {}", path.file_name().unwrap().to_string_lossy(), f.line, f.case, f.message)));
    }
    (checks, problems)
}

#[test]
fn specs_pass() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs");
    let (checks, problems) = run_dir(&dir);
    assert!(checks > 0, "no specs found in {dir:?}");
    assert!(problems.is_empty(), "{} of {checks} checks failed:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn pending_specs_are_reported() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/pending");
    let (checks, problems) = run_dir(&dir);
    println!("pending specs: {} of {checks} checks do not pass yet", problems.len());
    for p in &problems {
        println!("  {p}");
    }
}
