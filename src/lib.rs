mod discovery;
mod findings;
mod model;
mod render;

pub use model::{
    ActivationState, Agent, Analysis, Finding, FindingKind, ImportRef, ImportStatus,
    InstructionSource, Severity, SourceKind,
};
pub use render::{render_html, render_json, render_text};

use std::fs;
use std::io;
use std::path::Path;

pub fn analyze(root: &Path, target: Option<&Path>) -> io::Result<Analysis> {
    let root = fs::canonicalize(root)?;
    let target = target
        .map(|path| discovery::normalize_target(&root, path))
        .transpose()?;

    let discovered = discovery::discover(&root)?;
    let mut sources = match target.as_deref() {
        Some(target) => discovered
            .into_iter()
            .filter(|source| source.applies_to(target))
            .collect::<Vec<_>>(),
        None => discovered,
    };

    sources.sort_by(|a, b| {
        a.depth()
            .cmp(&b.depth())
            .then(crate::model::display_path(&a.path).cmp(&crate::model::display_path(&b.path)))
    });

    let findings = findings::detect_findings(&sources, target.as_deref());
    let total_bytes = sources.iter().map(|source| source.content.len()).sum::<usize>();
    let total_chars = sources
        .iter()
        .map(|source| source.content.chars().count())
        .sum::<usize>();
    let estimated_tokens = total_chars.div_ceil(4);

    Ok(Analysis {
        root,
        target,
        sources,
        findings,
        total_bytes,
        estimated_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "agentcontextmap-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn source<'a>(analysis: &'a Analysis, path: &str) -> &'a InstructionSource {
        analysis
            .sources
            .iter()
            .find(|source| crate::model::display_path(&source.path) == path)
            .unwrap_or_else(|| panic!("missing source {path}"))
    }

    #[test]
    fn target_resolves_documented_multi_agent_sources() {
        let root = temp_root("formats");
        write(&root, "AGENTS.md", "Always run tests.\n");
        write(&root, "src/AGENTS.md", "Use cargo test.\n");
        write(&root, "CLAUDE.md", "Prefer small changes.\n");
        write(&root, "GEMINI.md", "Prefer focused changes.\n");
        write(&root, ".github/copilot-instructions.md", "Use Rust 2021.\n");
        write(
            &root,
            ".github/instructions/rust.instructions.md",
            "---\napplyTo: \"**/*.rs\"\n---\nUse rustfmt.\n",
        );
        write(
            &root,
            ".cursor/rules/rust.mdc",
            "---\nglobs: \"**/*.rs\"\nalwaysApply: false\n---\nUse clippy.\n",
        );
        write(
            &root,
            ".windsurf/rules/rust.md",
            "---\ntrigger: glob\nglobs: **/*.rs\n---\nUse cargo check.\n",
        );
        write(
            &root,
            ".clinerules/rust.md",
            "---\npaths:\n  - \"**/*.rs\"\n---\nUse cargo test.\n",
        );
        write(&root, "src/lib.rs", "pub fn demo() {}\n");

        let analysis = analyze(&root, Some(Path::new("src/lib.rs"))).unwrap();
        for expected in [
            "AGENTS.md",
            "src/AGENTS.md",
            "CLAUDE.md",
            "GEMINI.md",
            ".github/copilot-instructions.md",
            ".github/instructions/rust.instructions.md",
            ".cursor/rules/rust.mdc",
            ".windsurf/rules/rust.md",
            ".clinerules/rust.md",
        ] {
            source(&analysis, expected);
        }
        let agents = &source(&analysis, "AGENTS.md").agents;
        assert!(agents.contains(&Agent::Codex));
        assert!(agents.contains(&Agent::Copilot));
        assert!(agents.contains(&Agent::Cursor));
        assert!(agents.contains(&Agent::Windsurf));
        assert!(agents.contains(&Agent::Cline));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_plain_markdown_rule_is_not_treated_as_mdc() {
        let root = temp_root("cursor-md");
        write(&root, ".cursor/rules/ignored.md", "Always test.\n");
        write(&root, ".cursor/rules/used.mdc", "---\nalwaysApply: true\n---\nAlways test.\n");
        let analysis = analyze(&root, None).unwrap();
        assert!(analysis.sources.iter().any(|source| source.path.ends_with("used.mdc")));
        assert!(!analysis.sources.iter().any(|source| source.path.ends_with("ignored.md")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn codex_override_is_discovered_separately() {
        let root = temp_root("override");
        write(&root, "AGENTS.md", "Always run tests.\n");
        write(&root, "AGENTS.override.md", "Never run tests.\n");
        let analysis = analyze(&root, Some(Path::new("src/lib.rs"))).unwrap();
        let override_source = source(&analysis, "AGENTS.override.md");
        assert_eq!(override_source.agents, vec![Agent::Codex]);
        assert!(analysis.findings.iter().any(|finding| finding.kind == FindingKind::Contradiction));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windsurf_activation_modes_are_preserved() {
        let root = temp_root("windsurf");
        write(&root, ".windsurf/rules/always.md", "---\ntrigger: always_on\n---\nAlways test.\n");
        write(&root, ".windsurf/rules/manual.md", "---\ntrigger: manual\n---\nDeploy carefully.\n");
        write(&root, ".windsurf/rules/model.md", "---\ntrigger: model_decision\ndescription: API guidance\n---\nUse typed errors.\n");
        let analysis = analyze(&root, Some(Path::new("src/lib.rs"))).unwrap();
        assert_eq!(source(&analysis, ".windsurf/rules/always.md").activation_state(analysis.target.as_deref()), ActivationState::Active);
        assert_eq!(source(&analysis, ".windsurf/rules/manual.md").activation_state(analysis.target.as_deref()), ActivationState::Manual);
        assert_eq!(source(&analysis, ".windsurf/rules/model.md").activation_state(analysis.target.as_deref()), ActivationState::Conditional);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cline_paths_are_target_specific() {
        let root = temp_root("cline");
        write(&root, ".clinerules/backend.md", "---\npaths:\n  - \"src/api/**\"\n---\nUse typed errors.\n");
        write(&root, "src/api/mod.rs", "pub fn api() {}\n");
        write(&root, "README.md", "hello\n");
        assert_eq!(analyze(&root, Some(Path::new("src/api/mod.rs"))).unwrap().sources.len(), 1);
        assert!(analyze(&root, Some(Path::new("README.md"))).unwrap().sources.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn claude_and_gemini_repository_imports_are_expanded_and_reported() {
        let root = temp_root("imports");
        write(&root, "CLAUDE.md", "Read @./docs/claude.md before changes.\n");
        write(&root, "docs/claude.md", "Always run cargo test.\n");
        write(&root, "GEMINI.md", "Use @./docs/gemini.md for style.\n");
        write(&root, "docs/gemini.md", "Prefer explicit types.\n");
        let analysis = analyze(&root, None).unwrap();
        let claude = source(&analysis, "CLAUDE.md");
        let gemini = source(&analysis, "GEMINI.md");
        assert!(claude.content.contains("Always run cargo test."));
        assert!(gemini.content.contains("Prefer explicit types."));
        assert!(claude.imports.iter().any(|import| import.status == ImportStatus::Loaded));
        assert!(gemini.imports.iter().any(|import| import.status == ImportStatus::Loaded));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_import_becomes_a_finding() {
        let root = temp_root("missing-import");
        write(&root, "CLAUDE.md", "Read @./docs/missing.md before changes.\n");
        let analysis = analyze(&root, None).unwrap();
        assert!(analysis.findings.iter().any(|finding| finding.kind == FindingKind::BrokenReference));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn target_cannot_escape_repository() {
        let root = temp_root("escape");
        let error = analyze(&root, Some(Path::new("../../etc/passwd"))).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        fs::remove_dir_all(root).unwrap();
    }
}
