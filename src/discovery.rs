use crate::model::{Agent, ImportRef, ImportStatus, InstructionSource, SourceKind};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub fn discover(root: &Path) -> io::Result<Vec<InstructionSource>> {
    let root = fs::canonicalize(root)?;
    let mut sources = Vec::new();
    walk(&root, &root, &mut sources)?;

    sources.sort_by(|a, b| {
        a.depth()
            .cmp(&b.depth())
            .then(source_priority(a).cmp(&source_priority(b)))
            .then(crate::model::display_path(&a.path).cmp(&crate::model::display_path(&b.path)))
    });
    Ok(sources)
}

pub fn normalize_target(root: &Path, target: &Path) -> io::Result<PathBuf> {
    if target.is_absolute() {
        let canonical = fs::canonicalize(target)?;
        return canonical
            .strip_prefix(root)
            .map(Path::to_path_buf)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "target is outside repository root",
                )
            });
    }

    let mut normalized = PathBuf::new();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "target escapes repository root",
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid target path",
                ));
            }
        }
    }
    Ok(normalized)
}

fn walk(root: &Path, dir: &Path, sources: &mut Vec<InstructionSource>) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            walk(root, &path, sources)?;
        } else if file_type.is_file() {
            if let Some(source) = detect_source(root, &path)? {
                sources.push(source);
            }
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git")
            | Some("target")
            | Some("node_modules")
            | Some("vendor")
            | Some(".venv")
            | Some("dist")
            | Some("build")
            | Some("coverage")
            | Some(".next")
    )
}

fn detect_source(root: &Path, path: &Path) -> io::Result<Option<InstructionSource>> {
    let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    let relative_string = crate::model::display_path(&relative);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if !is_supported_source(file_name, &relative_string) {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;

    let mut source = if file_name == "AGENTS.override.md" {
        hierarchical(relative, content, vec![Agent::Codex])
    } else if file_name == "AGENTS.md" {
        hierarchical(
            relative,
            content,
            vec![
                Agent::Codex,
                Agent::Copilot,
                Agent::Cursor,
                Agent::Windsurf,
                Agent::Cline,
            ],
        )
    } else if file_name == "agents.md" {
        let mut source = hierarchical(relative, content, vec![Agent::Windsurf]);
        source.notes.push("Lowercase agents.md is modeled for Windsurf only; other agents generally document AGENTS.md.".to_string());
        source
    } else if file_name == "CLAUDE.md" {
        hierarchical(relative, content, vec![Agent::Claude])
    } else if file_name == "GEMINI.md" {
        hierarchical(relative, content, vec![Agent::Gemini])
    } else if relative_string == ".github/copilot-instructions.md" {
        workspace(relative, content, vec![Agent::Copilot])
    } else if relative_string.starts_with(".github/instructions/")
        && relative_string.ends_with(".instructions.md")
    {
        let patterns = extract_frontmatter_list(&content, "applyTo");
        let mut source = pattern(relative, content, vec![Agent::Copilot], patterns);
        if source.patterns.is_empty() {
            source.notes.push(
                "Missing applyTo frontmatter; GitHub path-specific instructions require applyTo."
                    .to_string(),
            );
            source.kind = SourceKind::ModelDecision;
        }
        source
    } else if relative_string.starts_with(".cursor/rules/") && relative_string.ends_with(".mdc") {
        cursor_rule(relative, content)
    } else if relative_string.starts_with(".windsurf/rules/") && relative_string.ends_with(".md") {
        windsurf_rule(relative, content)
    } else if relative_string.starts_with(".clinerules/")
        && (relative_string.ends_with(".md") || relative_string.ends_with(".txt"))
    {
        cline_rule(relative, content)
    } else if relative_string == ".cursorrules" {
        let mut source = workspace(relative, content, vec![Agent::Cline]);
        source.notes.push(
            "Cline compatibility source: .cursorrules is auto-detected by Cline.".to_string(),
        );
        source
    } else if relative_string == ".windsurfrules" {
        let mut source = workspace(relative, content, vec![Agent::Cline]);
        source.notes.push("Cline compatibility source: .windsurfrules is auto-detected by Cline. Current Windsurf rules use .windsurf/rules/*.md.".to_string());
        source
    } else {
        return Ok(None);
    };

    if matches!(file_name, "CLAUDE.md" | "GEMINI.md") {
        expand_repository_imports(root, path, &mut source)?;
    }

    Ok(Some(source))
}

fn is_supported_source(file_name: &str, relative: &str) -> bool {
    matches!(
        file_name,
        "AGENTS.override.md" | "AGENTS.md" | "agents.md" | "CLAUDE.md" | "GEMINI.md"
    ) || relative == ".github/copilot-instructions.md"
        || (relative.starts_with(".github/instructions/") && relative.ends_with(".instructions.md"))
        || (relative.starts_with(".cursor/rules/") && relative.ends_with(".mdc"))
        || (relative.starts_with(".windsurf/rules/") && relative.ends_with(".md"))
        || (relative.starts_with(".clinerules/")
            && (relative.ends_with(".md") || relative.ends_with(".txt")))
        || relative == ".cursorrules"
        || relative == ".windsurfrules"
}

fn hierarchical(path: PathBuf, content: String, agents: Vec<Agent>) -> InstructionSource {
    let scope = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    InstructionSource {
        bytes: content.len(),
        path,
        agents,
        kind: SourceKind::Hierarchical,
        scope,
        patterns: Vec::new(),
        content,
        imports: Vec::new(),
        notes: Vec::new(),
    }
}

fn workspace(path: PathBuf, content: String, agents: Vec<Agent>) -> InstructionSource {
    InstructionSource {
        bytes: content.len(),
        path,
        agents,
        kind: SourceKind::Workspace,
        scope: PathBuf::new(),
        patterns: Vec::new(),
        content,
        imports: Vec::new(),
        notes: Vec::new(),
    }
}

fn pattern(
    path: PathBuf,
    content: String,
    agents: Vec<Agent>,
    patterns: Vec<String>,
) -> InstructionSource {
    InstructionSource {
        bytes: content.len(),
        path,
        agents,
        kind: SourceKind::Pattern,
        scope: PathBuf::new(),
        patterns,
        content,
        imports: Vec::new(),
        notes: Vec::new(),
    }
}

fn cursor_rule(path: PathBuf, content: String) -> InstructionSource {
    let globs = extract_frontmatter_list(&content, "globs");
    let always_apply = extract_frontmatter_scalar(&content, "alwaysApply")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let description = extract_frontmatter_scalar(&content, "description");

    if always_apply {
        workspace(path, content, vec![Agent::Cursor])
    } else if !globs.is_empty() {
        pattern(path, content, vec![Agent::Cursor], globs)
    } else if description.is_some() {
        let mut source = workspace(path, content, vec![Agent::Cursor]);
        source.kind = SourceKind::ModelDecision;
        source.notes.push("Cursor rule has a description but no globs/alwaysApply; modeled as agent-decided relevance.".to_string());
        source
    } else {
        let mut source = workspace(path, content, vec![Agent::Cursor]);
        source.kind = SourceKind::Manual;
        source.notes.push(
            "Cursor rule has no automatic activation metadata; modeled as manual.".to_string(),
        );
        source
    }
}

fn windsurf_rule(path: PathBuf, content: String) -> InstructionSource {
    let trigger = extract_frontmatter_scalar(&content, "trigger").unwrap_or_default();
    let globs = extract_frontmatter_list(&content, "globs");
    match trigger.as_str() {
        "always_on" => workspace(path, content, vec![Agent::Windsurf]),
        "glob" => {
            let mut source = pattern(path, content, vec![Agent::Windsurf], globs);
            if source.patterns.is_empty() {
                source.kind = SourceKind::ModelDecision;
                source.notes.push(
                    "Windsurf glob rule is missing globs; activation cannot be resolved exactly."
                        .to_string(),
                );
            }
            source
        }
        "model_decision" => {
            let mut source = workspace(path, content, vec![Agent::Windsurf]);
            source.kind = SourceKind::ModelDecision;
            source
        }
        "manual" => {
            let mut source = workspace(path, content, vec![Agent::Windsurf]);
            source.kind = SourceKind::Manual;
            source
        }
        _ => {
            let mut source = workspace(path, content, vec![Agent::Windsurf]);
            source.kind = SourceKind::ModelDecision;
            source.notes.push("Windsurf rule is missing a recognized trigger; modeled conservatively as conditional.".to_string());
            source
        }
    }
}

fn cline_rule(path: PathBuf, content: String) -> InstructionSource {
    let paths = extract_frontmatter_list(&content, "paths");
    if paths.is_empty() {
        workspace(path, content, vec![Agent::Cline])
    } else {
        pattern(path, content, vec![Agent::Cline], paths)
    }
}

fn source_priority(source: &InstructionSource) -> u8 {
    if source.path.file_name().and_then(|name| name.to_str()) == Some("AGENTS.override.md") {
        2
    } else {
        1
    }
}

fn frontmatter_lines(content: &str) -> Option<Vec<&str>> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut result = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            return Some(result);
        }
        result.push(line);
    }
    None
}

pub fn extract_frontmatter_scalar(content: &str, key: &str) -> Option<String> {
    let lines = frontmatter_lines(content)?;
    for line in lines {
        let trimmed = line.trim();
        let Some((candidate, value)) = trimmed.split_once(':') else {
            continue;
        };
        if candidate.trim() == key {
            let value = strip_quotes(value.trim());
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn extract_frontmatter_list(content: &str, key: &str) -> Vec<String> {
    let Some(lines) = frontmatter_lines(content) else {
        return Vec::new();
    };

    let mut collecting = false;
    let mut result = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if collecting {
            if let Some(item) = trimmed.strip_prefix('-') {
                let item = strip_quotes(item.trim());
                if !item.is_empty() {
                    result.push(item.to_string());
                }
                continue;
            }
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                collecting = false;
            }
        }

        let Some((candidate, value)) = trimmed.split_once(':') else {
            continue;
        };
        if candidate.trim() != key {
            continue;
        }

        let value = value.trim();
        if value.is_empty() {
            collecting = true;
            continue;
        }

        let inline = value.trim_matches('[').trim_matches(']');
        for item in split_patterns(inline) {
            if !item.is_empty() {
                result.push(item);
            }
        }
    }
    result
}

fn split_patterns(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| strip_quotes(item.trim()).trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn strip_quotes(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'').trim()
}

fn expand_repository_imports(
    root: &Path,
    source_path: &Path,
    source: &mut InstructionSource,
) -> io::Result<()> {
    let mut seen = HashSet::new();
    seen.insert(fs::canonicalize(source_path)?);
    let mut expanded = source.content.clone();
    let mut imports = Vec::new();
    expand_imports_recursive(
        root,
        source_path,
        &source.content,
        0,
        &mut seen,
        &mut imports,
        &mut expanded,
    )?;
    source.content = expanded;
    source.imports = imports;
    Ok(())
}

fn expand_imports_recursive(
    root: &Path,
    parent: &Path,
    content: &str,
    depth: usize,
    seen: &mut HashSet<PathBuf>,
    imports: &mut Vec<ImportRef>,
    expanded: &mut String,
) -> io::Result<()> {
    for token in import_tokens(content) {
        let raw = token.trim_start_matches('@');
        if raw.starts_with('~') || Path::new(raw).is_absolute() {
            imports.push(ImportRef {
                path: PathBuf::from(raw),
                status: ImportStatus::OutsideRepository,
                bytes: 0,
            });
            continue;
        }

        if depth >= 5 {
            imports.push(ImportRef {
                path: PathBuf::from(raw),
                status: ImportStatus::DepthLimit,
                bytes: 0,
            });
            continue;
        }

        let candidate = parent.parent().unwrap_or(root).join(raw);
        let canonical = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(_) => {
                imports.push(ImportRef {
                    path: candidate
                        .strip_prefix(root)
                        .unwrap_or(&candidate)
                        .to_path_buf(),
                    status: ImportStatus::Missing,
                    bytes: 0,
                });
                continue;
            }
        };

        if !canonical.starts_with(root) {
            imports.push(ImportRef {
                path: canonical,
                status: ImportStatus::OutsideRepository,
                bytes: 0,
            });
            continue;
        }
        if !seen.insert(canonical.clone()) {
            continue;
        }

        let imported = match fs::read_to_string(&canonical) {
            Ok(content) => content,
            Err(_) => {
                imports.push(ImportRef {
                    path: canonical
                        .strip_prefix(root)
                        .unwrap_or(&canonical)
                        .to_path_buf(),
                    status: ImportStatus::Missing,
                    bytes: 0,
                });
                continue;
            }
        };
        let relative = canonical
            .strip_prefix(root)
            .unwrap_or(&canonical)
            .to_path_buf();
        imports.push(ImportRef {
            path: relative.clone(),
            status: ImportStatus::Loaded,
            bytes: imported.len(),
        });
        expanded.push_str("\n\n# Imported by AgentContextMap: ");
        expanded.push_str(&crate::model::display_path(&relative));
        expanded.push('\n');
        expanded.push_str(&imported);
        expand_imports_recursive(
            root,
            &canonical,
            &imported,
            depth + 1,
            seen,
            imports,
            expanded,
        )?;
    }
    Ok(())
}

fn import_tokens(content: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_fence = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for word in line.split_whitespace() {
            if !word.starts_with('@') || word.len() < 2 {
                continue;
            }
            let cleaned = word
                .trim_matches(|ch: char| {
                    matches!(ch, ',' | ';' | ':' | ')' | ']' | '}' | '"' | '\'')
                })
                .to_string();
            if cleaned.contains('.') || cleaned.contains('/') || cleaned.starts_with("@README") {
                result.push(cleaned);
            }
        }
    }
    result
}

pub fn glob_matches(pattern: &str, target: &Path) -> bool {
    let text = crate::model::display_path(target);
    brace_expand(pattern)
        .into_iter()
        .any(|candidate| glob_match_one(candidate.trim_start_matches("./"), &text))
}

fn brace_expand(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let Some(close_rel) = pattern[open + 1..].find('}') else {
        return vec![pattern.to_string()];
    };
    let close = open + 1 + close_rel;
    let before = &pattern[..open];
    let inside = &pattern[open + 1..close];
    let after = &pattern[close + 1..];
    let mut result = Vec::new();
    for choice in inside.split(',') {
        for expanded in brace_expand(&format!("{before}{choice}{after}")) {
            result.push(expanded);
        }
    }
    result
}

fn glob_match_one(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let mut memo = HashSet::new();
    glob_step(p, t, 0, 0, &mut memo)
}

fn glob_step(
    pattern: &[u8],
    text: &[u8],
    pi: usize,
    ti: usize,
    failed: &mut HashSet<(usize, usize)>,
) -> bool {
    if failed.contains(&(pi, ti)) {
        return false;
    }
    if pi == pattern.len() {
        return ti == text.len();
    }

    let matched = if pattern[pi] == b'*' {
        let double = pi + 1 < pattern.len() && pattern[pi + 1] == b'*';
        if double {
            let mut next = pi + 2;
            while next < pattern.len() && pattern[next] == b'*' {
                next += 1;
            }
            if next < pattern.len() && pattern[next] == b'/' {
                glob_step(pattern, text, next + 1, ti, failed)
                    || (ti < text.len() && glob_step(pattern, text, pi, ti + 1, failed))
            } else {
                glob_step(pattern, text, next, ti, failed)
                    || (ti < text.len() && glob_step(pattern, text, pi, ti + 1, failed))
            }
        } else {
            glob_step(pattern, text, pi + 1, ti, failed)
                || (ti < text.len()
                    && text[ti] != b'/'
                    && glob_step(pattern, text, pi, ti + 1, failed))
        }
    } else if pattern[pi] == b'?' {
        ti < text.len() && text[ti] != b'/' && glob_step(pattern, text, pi + 1, ti + 1, failed)
    } else if pattern[pi] == b'[' {
        match_class(pattern, text, pi, ti, failed)
    } else {
        ti < text.len()
            && pattern[pi] == text[ti]
            && glob_step(pattern, text, pi + 1, ti + 1, failed)
    };

    if !matched {
        failed.insert((pi, ti));
    }
    matched
}

fn match_class(
    pattern: &[u8],
    text: &[u8],
    pi: usize,
    ti: usize,
    failed: &mut HashSet<(usize, usize)>,
) -> bool {
    if ti >= text.len() || text[ti] == b'/' {
        return false;
    }
    let Some(end_rel) = pattern[pi + 1..].iter().position(|byte| *byte == b']') else {
        return false;
    };
    let end = pi + 1 + end_rel;
    let class = &pattern[pi + 1..end];
    let mut allowed = false;
    let mut index = 0usize;
    while index < class.len() {
        if index + 2 < class.len() && class[index + 1] == b'-' {
            if text[ti] >= class[index] && text[ti] <= class[index + 2] {
                allowed = true;
            }
            index += 3;
        } else {
            if text[ti] == class[index] {
                allowed = true;
            }
            index += 1;
        }
    }
    allowed && glob_step(pattern, text, end + 1, ti + 1, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_star_does_not_cross_directories() {
        assert!(glob_matches("*.rs", Path::new("lib.rs")));
        assert!(!glob_matches("*.rs", Path::new("src/lib.rs")));
        assert!(glob_matches("**/*.rs", Path::new("src/lib.rs")));
        assert!(glob_matches("**/*.rs", Path::new("lib.rs")));
    }

    #[test]
    fn glob_supports_braces_and_character_classes() {
        assert!(glob_matches("src/*.{ts,tsx}", Path::new("src/app.tsx")));
        assert!(glob_matches("src/[ab].rs", Path::new("src/a.rs")));
        assert!(!glob_matches("src/[ab].rs", Path::new("src/c.rs")));
    }

    #[test]
    fn frontmatter_parses_inline_and_block_lists() {
        let inline = "---\nglobs: [\"**/*.ts\", \"src/**\"]\n---\nbody";
        assert_eq!(extract_frontmatter_list(inline, "globs").len(), 2);
        let block = "---\npaths:\n  - \"src/**\"\n  - tests/**\n---\nbody";
        assert_eq!(
            extract_frontmatter_list(block, "paths"),
            vec!["src/**", "tests/**"]
        );
    }

    #[test]
    fn discovery_ignores_unrelated_non_utf8_files() {
        let root = std::env::temp_dir().join(format!(
            "agentcontextmap-discovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp repo");
        fs::write(root.join("AGENTS.md"), "Always run tests.\n").expect("write AGENTS.md");
        fs::write(root.join("logo.bin"), [0xff, 0xfe, 0xfd]).expect("write binary");

        let sources = discover(&root).expect("unrelated binary files must be ignored");
        fs::remove_dir_all(&root).expect("cleanup temp repo");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path, PathBuf::from("AGENTS.md"));
    }
}
