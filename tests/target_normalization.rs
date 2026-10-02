use agentcontextmap::analyze;
use std::fs;
use std::path::Path;

#[test]
fn normalizes_safe_parent_components_inside_repository() {
    let root = tempfile_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("AGENTS.md"), "Always run tests.\n").unwrap();
    fs::write(root.join("README.md"), "hello\n").unwrap();

    let analysis = analyze(&root, Some(Path::new("src/../README.md"))).unwrap();

    assert_eq!(analysis.target.as_deref(), Some(Path::new("README.md")));
    assert_eq!(analysis.sources.len(), 1);

    let _ = fs::remove_dir_all(root);
}

fn tempfile_root() -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agentcontext-target-normalization-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
