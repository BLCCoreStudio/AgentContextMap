use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Agent {
    Codex,
    Claude,
    Gemini,
    Copilot,
    Cursor,
    Windsurf,
    Cline,
}

impl Agent {
    pub fn label(self) -> &'static str {
        match self {
            Agent::Codex => "Codex / AGENTS.md",
            Agent::Claude => "Claude Code",
            Agent::Gemini => "Gemini",
            Agent::Copilot => "GitHub Copilot",
            Agent::Cursor => "Cursor",
            Agent::Windsurf => "Windsurf",
            Agent::Cline => "Cline",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Agent::Codex => "codex",
            Agent::Claude => "claude",
            Agent::Gemini => "gemini",
            Agent::Copilot => "copilot",
            Agent::Cursor => "cursor",
            Agent::Windsurf => "windsurf",
            Agent::Cline => "cline",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Hierarchical,
    Workspace,
    Pattern,
}

#[derive(Debug, Clone)]
pub struct InstructionSource {
    pub path: PathBuf,
    pub agent: Agent,
    pub kind: SourceKind,
    pub scope: PathBuf,
    pub pattern: Option<String>,
    pub bytes: usize,
    pub content: String,
}

impl InstructionSource {
    pub fn applies_to(&self, target: &Path) -> bool {
        match self.kind {
            SourceKind::Hierarchical => target.starts_with(&self.scope),
            SourceKind::Workspace => true,
            SourceKind::Pattern => self
                .pattern
                .as_deref()
                .map(|pattern| glob_list_matches(pattern, target))
                .unwrap_or(false),
        }
    }

    fn depth(&self) -> usize {
        self.scope.components().count()
    }

    fn scope_label(&self) -> String {
        match self.kind {
            SourceKind::Hierarchical => {
                if self.scope.as_os_str().is_empty() {
                    "workspace tree".to_string()
                } else {
                    format!("{} subtree", display_path(&self.scope))
                }
            }
            SourceKind::Workspace => "workspace-wide".to_string(),
            SourceKind::Pattern => self
                .pattern
                .as_ref()
                .map(|p| format!("pattern: {p}"))
                .unwrap_or_else(|| "manual / conditional".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    Contradiction,
    ChoiceConflict,
    Duplicate,
}

impl FindingKind {
    pub fn label(self) -> &'static str {
        match self {
            FindingKind::Contradiction => "CONFLICT",
            FindingKind::ChoiceConflict => "CHOICE",
            FindingKind::Duplicate => "DUPLICATE",
        }
    }

    pub fn severity(self) -> &'static str {
        match self {
            FindingKind::Contradiction | FindingKind::ChoiceConflict => "high",
            FindingKind::Duplicate => "low",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: FindingKind,
    pub left_source: PathBuf,
    pub right_source: PathBuf,
    pub left_line: String,
    pub right_line: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub root: PathBuf,
    pub target: Option<PathBuf>,
    pub sources: Vec<InstructionSource>,
    pub findings: Vec<Finding>,
    pub total_bytes: usize,
    pub estimated_tokens: usize,
}

pub fn analyze(root: &Path, target: Option<&Path>) -> io::Result<Analysis> {
    let root = fs::canonicalize(root)?;
    let discovered = discover(&root)?;
    let target = target.map(normalize_relative_path);

    let mut sources: Vec<InstructionSource> = match target.as_deref() {
        Some(target) => discovered
            .into_iter()
            .filter(|source| source.applies_to(target))
            .collect(),
        None => discovered,
    };

    sources.sort_by(|a, b| {
        a.agent
            .cmp(&b.agent)
            .then(a.depth().cmp(&b.depth()))
            .then(display_path(&a.path).cmp(&display_path(&b.path)))
    });

    let findings = detect_findings(&sources);
    let total_bytes = sources.iter().map(|source| source.bytes).sum::<usize>();
    let total_chars = sources
        .iter()
        .map(|source| source.content.chars().count())
        .sum::<usize>();
    let estimated_tokens = (total_chars + 3) / 4;

    Ok(Analysis {
        root,
        target,
        sources,
        findings,
        total_bytes,
        estimated_tokens,
    })
}

pub fn discover(root: &Path) -> io::Result<Vec<InstructionSource>> {
    let root = fs::canonicalize(root)?;
    let mut sources = Vec::new();
    walk(&root, &root, &mut sources)?;
    sources.sort_by(|a, b| display_path(&a.path).cmp(&display_path(&b.path)));
    Ok(sources)
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
    )
}

fn detect_source(root: &Path, path: &Path) -> io::Result<Option<InstructionSource>> {
    let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    let relative_string = display_path(&relative);
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");

    let identity = if file_name == "AGENTS.md" {
        Some((Agent::Codex, SourceKind::Hierarchical))
    } else if file_name == "CLAUDE.md" {
        Some((Agent::Claude, SourceKind::Hierarchical))
    } else if file_name == "GEMINI.md" {
        Some((Agent::Gemini, SourceKind::Hierarchical))
    } else if relative_string == ".github/copilot-instructions.md" {
        Some((Agent::Copilot, SourceKind::Workspace))
    } else if relative_string.starts_with(".github/instructions/")
        && relative_string.ends_with(".instructions.md")
    {
        Some((Agent::Copilot, SourceKind::Pattern))
    } else if relative_string.starts_with(".cursor/rules/")
        && (relative_string.ends_with(".mdc") || relative_string.ends_with(".md"))
    {
        Some((Agent::Cursor, SourceKind::Pattern))
    } else if file_name == ".windsurfrules" {
        Some((Agent::Windsurf, SourceKind::Workspace))
    } else if file_name == ".clinerules" {
        Some((Agent::Cline, SourceKind::Workspace))
    } else {
        None
    };

    let Some((agent, mut kind)) = identity else {
        return Ok(None);
    };

    let content = fs::read_to_string(path)?;
    let bytes = content.len();
    let scope = if kind == SourceKind::Hierarchical {
        relative
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf()
    } else {
        PathBuf::new()
    };

    let mut pattern = None;

    if agent == Agent::Copilot && kind == SourceKind::Pattern {
        pattern = extract_frontmatter_value(&content, "applyTo");
    }

    if agent == Agent::Cursor && kind == SourceKind::Pattern {
        pattern = extract_frontmatter_value(&content, "globs");
        let always_apply = extract_frontmatter_value(&content, "alwaysApply")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if always_apply {
            kind = SourceKind::Workspace;
            pattern = None;
        }
    }

    Ok(Some(InstructionSource {
        path: relative,
        agent,
        kind,
        scope,
        pattern,
        bytes,
        content,
    }))
}

fn extract_frontmatter_value(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((candidate, value)) = trimmed.split_once(':') else {
            continue;
        };
        if candidate.trim() == key {
            let value = value
                .trim()
                .trim_matches('[')
                .trim_matches(']')
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }

    None
}

fn normalize_relative_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    normalized
}

fn glob_list_matches(patterns: &str, target: &Path) -> bool {
    let target = display_path(target);
    patterns
        .split(',')
        .map(clean_pattern)
        .filter(|pattern| !pattern.is_empty())
        .any(|pattern| {
            wildcard_match(&pattern, &target)
                || pattern
                    .strip_prefix("**/")
                    .map(|short| wildcard_match(short, &target))
                    .unwrap_or(false)
        })
}

fn clean_pattern(pattern: &str) -> String {
    pattern
        .trim()
        .trim_matches('[')
        .trim_matches(']')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .trim_start_matches("./")
        .to_string()
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let (mut p, mut t) = (0usize, 0usize);
    let mut star = None;
    let mut match_after_star = 0usize;

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            match_after_star = t;
        } else if let Some(star_index) = star {
            p = star_index + 1;
            match_after_star += 1;
            t = match_after_star;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }

    p == pattern.len()
}

#[derive(Debug)]
struct Directive {
    source_index: usize,
    line: String,
    normalized: String,
    polarity: i8,
    keywords: HashSet<String>,
    choice: Option<&'static str>,
}

fn detect_findings(sources: &[InstructionSource]) -> Vec<Finding> {
    let directives = collect_directives(sources);
    let mut findings = Vec::new();
    let mut seen = HashSet::new();

    for left_index in 0..directives.len() {
        for right_index in (left_index + 1)..directives.len() {
            let left = &directives[left_index];
            let right = &directives[right_index];
            let left_source = &sources[left.source_index];
            let right_source = &sources[right.source_index];

            if left.source_index == right.source_index || !sources_can_overlap(left_source, right_source) {
                continue;
            }

            if left.normalized == right.normalized {
                let key = format!(
                    "dup:{}:{}:{}",
                    display_path(&left_source.path),
                    display_path(&right_source.path),
                    left.normalized
                );
                if seen.insert(key) {
                    findings.push(Finding {
                        kind: FindingKind::Duplicate,
                        left_source: left_source.path.clone(),
                        right_source: right_source.path.clone(),
                        left_line: left.line.clone(),
                        right_line: right.line.clone(),
                        summary: "The same directive appears in overlapping instruction sources."
                            .to_string(),
                    });
                }
                continue;
            }

            if let (Some(left_choice), Some(right_choice)) = (left.choice, right.choice) {
                if left_choice != right_choice {
                    let key = format!(
                        "choice:{}:{}:{}:{}",
                        display_path(&left_source.path),
                        display_path(&right_source.path),
                        left_choice,
                        right_choice
                    );
                    if seen.insert(key) {
                        findings.push(Finding {
                            kind: FindingKind::ChoiceConflict,
                            left_source: left_source.path.clone(),
                            right_source: right_source.path.clone(),
                            left_line: left.line.clone(),
                            right_line: right.line.clone(),
                            summary: format!(
                                "Different JavaScript package managers are required: {left_choice} vs {right_choice}."
                            ),
                        });
                    }
                    continue;
                }
            }

            if left.polarity != 0
                && right.polarity != 0
                && left.polarity != right.polarity
                && keyword_overlap(&left.keywords, &right.keywords) >= 0.5
            {
                let key = format!(
                    "conflict:{}:{}:{}:{}",
                    display_path(&left_source.path),
                    display_path(&right_source.path),
                    left.normalized,
                    right.normalized
                );
                if seen.insert(key) {
                    findings.push(Finding {
                        kind: FindingKind::Contradiction,
                        left_source: left_source.path.clone(),
                        right_source: right_source.path.clone(),
                        left_line: left.line.clone(),
                        right_line: right.line.clone(),
                        summary: "Overlapping sources contain directives with opposite polarity."
                            .to_string(),
                    });
                }
            }
        }
    }

    findings.sort_by(|a, b| {
        a.kind
            .severity()
            .cmp(b.kind.severity())
            .then(display_path(&a.left_source).cmp(&display_path(&b.left_source)))
    });
    findings
}

fn collect_directives(sources: &[InstructionSource]) -> Vec<Directive> {
    let mut directives = Vec::new();

    for (source_index, source) in sources.iter().enumerate() {
        for raw_line in source.content.lines() {
            let line = strip_markdown_prefix(raw_line);
            if line.len() < 4 || line.len() > 240 {
                continue;
            }

            let normalized = normalize_text(&line);
            let polarity = detect_polarity(&normalized);
            let choice = detect_package_manager(&normalized);
            if polarity == 0 && choice.is_none() {
                continue;
            }

            directives.push(Directive {
                source_index,
                line,
                keywords: keyword_set(&normalized),
                normalized,
                polarity,
                choice,
            });
        }
    }

    directives
}

fn strip_markdown_prefix(line: &str) -> String {
    line.trim()
        .trim_start_matches(|c: char| matches!(c, '-' | '*' | '+' | '#' | '>' | ' ' | '\t'))
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')' || c == ' ')
        .trim()
        .to_string()
}

fn normalize_text(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_space = false;

    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            normalized.push(ch);
            previous_space = false;
        } else if !previous_space {
            normalized.push(' ');
            previous_space = true;
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn detect_polarity(line: &str) -> i8 {
    let negative = [
        "do not ",
        "don't ",
        "dont ",
        "never ",
        "must not ",
        "forbidden",
        "forbid ",
        "avoid ",
        "disallow ",
        "disabled",
        "without ",
    ];
    if negative.iter().any(|needle| line.contains(needle)) {
        return -1;
    }

    let positive = [
        "must ",
        "always ",
        "required",
        "require ",
        "should ",
        "use ",
        "enable ",
        "allow ",
        "run ",
    ];
    if positive.iter().any(|needle| line.contains(needle)) {
        return 1;
    }

    0
}

fn detect_package_manager(line: &str) -> Option<&'static str> {
    let candidates = ["pnpm", "yarn", "npm", "bun"];
    candidates
        .into_iter()
        .find(|candidate| contains_word(line, candidate))
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack.split_whitespace().any(|word| word == needle)
}

fn keyword_set(line: &str) -> HashSet<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "before", "by", "do", "dont", "for",
        "from", "in", "is", "it", "must", "never", "not", "of", "on", "or", "should", "the",
        "this", "to", "use", "always", "required", "require", "without", "with",
    ];

    line.split_whitespace()
        .filter(|word| word.len() > 2 && !STOPWORDS.contains(word))
        .map(ToString::to_string)
        .collect()
}

fn keyword_overlap(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    let denominator = left.len().min(right.len());
    if denominator == 0 {
        return 0.0;
    }
    let intersection = left.intersection(right).count();
    intersection as f32 / denominator as f32
}

fn sources_can_overlap(left: &InstructionSource, right: &InstructionSource) -> bool {
    match (left.kind, right.kind) {
        (SourceKind::Hierarchical, SourceKind::Hierarchical) => {
            left.scope.starts_with(&right.scope) || right.scope.starts_with(&left.scope)
        }
        _ => true,
    }
}

pub fn render_text(analysis: &Analysis) -> String {
    let mut out = String::new();
    out.push_str("AgentContextMap\n");
    out.push_str("===============\n");
    out.push_str(&format!("Root: {}\n", analysis.root.display()));
    match analysis.target.as_deref() {
        Some(target) => out.push_str(&format!("Target: {}\n", display_path(target))),
        None => out.push_str("Target: workspace overview\n"),
    }
    out.push_str(&format!(
        "Sources: {} | Approx. tokens: {} | Findings: {}\n\n",
        analysis.sources.len(),
        analysis.estimated_tokens,
        analysis.findings.len()
    ));

    if analysis.sources.is_empty() {
        out.push_str("No supported repository instruction files found.\n");
        return out;
    }

    let mut grouped: BTreeMap<Agent, Vec<&InstructionSource>> = BTreeMap::new();
    for source in &analysis.sources {
        grouped.entry(source.agent).or_default().push(source);
    }

    for (agent, sources) in grouped {
        out.push_str(&format!("[{}]\n", agent.label()));
        for (index, source) in sources.iter().enumerate() {
            out.push_str(&format!(
                "  {}. {}  ({})\n",
                index + 1,
                display_path(&source.path),
                source.scope_label()
            ));
        }
        out.push('\n');
    }

    if analysis.findings.is_empty() {
        out.push_str("Findings: none detected by the current deterministic heuristics.\n");
    } else {
        out.push_str("Findings\n--------\n");
        for finding in &analysis.findings {
            out.push_str(&format!(
                "{} [{}] {} <-> {}\n",
                finding.kind.label(),
                finding.kind.severity(),
                display_path(&finding.left_source),
                display_path(&finding.right_source)
            ));
            out.push_str(&format!("  {}\n", finding.summary));
            out.push_str(&format!("  - {}\n", finding.left_line));
            out.push_str(&format!("  - {}\n", finding.right_line));
        }
    }

    out
}

pub fn render_json(analysis: &Analysis) -> String {
    let mut json = String::new();
    json.push('{');
    json.push_str(&format!("\"root\":\"{}\",", json_escape(&analysis.root.display().to_string())));
    match analysis.target.as_deref() {
        Some(target) => json.push_str(&format!(
            "\"target\":\"{}\",",
            json_escape(&display_path(target))
        )),
        None => json.push_str("\"target\":null,"),
    }
    json.push_str(&format!(
        "\"source_count\":{},\"estimated_tokens\":{},\"sources\":[",
        analysis.sources.len(),
        analysis.estimated_tokens
    ));

    for (index, source) in analysis.sources.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"agent\":\"{}\",\"path\":\"{}\",\"scope\":\"{}\",\"pattern\":{},\"bytes\":{}}}",
            json_escape(source.agent.label()),
            json_escape(&display_path(&source.path)),
            json_escape(&source.scope_label()),
            source
                .pattern
                .as_ref()
                .map(|pattern| format!("\"{}\"", json_escape(pattern)))
                .unwrap_or_else(|| "null".to_string()),
            source.bytes
        ));
    }

    json.push_str("],\"findings\":[");
    for (index, finding) in analysis.findings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"kind\":\"{}\",\"severity\":\"{}\",\"left_source\":\"{}\",\"right_source\":\"{}\",\"left_line\":\"{}\",\"right_line\":\"{}\",\"summary\":\"{}\"}}",
            finding.kind.label(),
            finding.kind.severity(),
            json_escape(&display_path(&finding.left_source)),
            json_escape(&display_path(&finding.right_source)),
            json_escape(&finding.left_line),
            json_escape(&finding.right_line),
            json_escape(&finding.summary)
        ));
    }
    json.push_str("]}");
    json
}

pub fn render_html(analysis: &Analysis) -> String {
    let mut source_cards = String::new();
    for source in &analysis.sources {
        source_cards.push_str(&format!(
            "<article class=\"source\" data-agent=\"{}\"><div class=\"source-top\"><span class=\"agent\">{}</span><span class=\"bytes\">{} bytes</span></div><h3>{}</h3><p>{}</p></article>",
            source.agent.slug(),
            html_escape(source.agent.label()),
            source.bytes,
            html_escape(&display_path(&source.path)),
            html_escape(&source.scope_label())
        ));
    }

    let mut finding_cards = String::new();
    if analysis.findings.is_empty() {
        finding_cards.push_str("<div class=\"empty\">No conflicts or duplicates detected by the current deterministic heuristics.</div>");
    } else {
        for finding in &analysis.findings {
            finding_cards.push_str(&format!(
                "<article class=\"finding {}\"><div><strong>{}</strong><span>{}</span></div><p>{}</p><code>{}</code><code>{}</code><small>{} ↔ {}</small></article>",
                finding.kind.severity(),
                finding.kind.label(),
                finding.kind.severity(),
                html_escape(&finding.summary),
                html_escape(&finding.left_line),
                html_escape(&finding.right_line),
                html_escape(&display_path(&finding.left_source)),
                html_escape(&display_path(&finding.right_source))
            ));
        }
    }

    let target = analysis
        .target
        .as_deref()
        .map(display_path)
        .unwrap_or_else(|| "workspace overview".to_string());

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>AgentContextMap report</title>
<style>
:root{{--bg:#0b1020;--panel:#121a2f;--panel2:#18223b;--text:#eef3ff;--muted:#9eabc7;--line:#2a3655;--accent:#8ab4ff;--good:#7ee2b8;--warn:#ffd580;--bad:#ff8f9c}}
*{{box-sizing:border-box}} body{{margin:0;background:linear-gradient(180deg,#0b1020,#0d1324 45%,#090d18);color:var(--text);font:15px/1.5 ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
.wrap{{max-width:1180px;margin:auto;padding:48px 22px 72px}} .eyebrow{{color:var(--accent);font-weight:700;letter-spacing:.08em;text-transform:uppercase;font-size:12px}} h1{{font-size:clamp(34px,6vw,64px);line-height:1;margin:10px 0 14px;letter-spacing:-.04em}} .sub{{color:var(--muted);font-size:18px;max-width:760px}}
.metrics{{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin:28px 0}} .metric{{background:rgba(18,26,47,.8);border:1px solid var(--line);border-radius:16px;padding:18px}} .metric strong{{font-size:28px;display:block}} .metric span{{color:var(--muted)}}
.toolbar{{display:flex;flex-wrap:wrap;gap:8px;margin:24px 0 18px}} button{{border:1px solid var(--line);background:var(--panel);color:var(--text);padding:9px 13px;border-radius:999px;cursor:pointer}} button:hover,button.active{{border-color:var(--accent);color:var(--accent)}}
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(270px,1fr));gap:12px}} .source{{background:var(--panel);border:1px solid var(--line);border-radius:16px;padding:17px;min-height:130px}} .source[hidden]{{display:none}} .source-top{{display:flex;justify-content:space-between;gap:12px}} .agent{{color:var(--accent);font-weight:700}} .bytes{{color:var(--muted);font-size:12px}} .source h3{{margin:18px 0 6px;font-size:16px;word-break:break-word}} .source p{{margin:0;color:var(--muted)}}
section{{margin-top:44px}} h2{{font-size:24px;margin-bottom:14px}} .finding{{background:var(--panel);border:1px solid var(--line);border-left:4px solid var(--bad);border-radius:14px;padding:16px;margin:10px 0}} .finding.low{{border-left-color:var(--warn)}} .finding>div{{display:flex;justify-content:space-between}} .finding span{{color:var(--muted);text-transform:uppercase;font-size:11px}} .finding code{{display:block;background:var(--panel2);padding:10px;border-radius:9px;margin:7px 0;white-space:pre-wrap;color:#d9e2f7}} .finding small{{color:var(--muted)}} .empty{{color:var(--muted);border:1px dashed var(--line);padding:18px;border-radius:14px}}
footer{{margin-top:52px;color:var(--muted);font-size:13px}} @media(max-width:640px){{.metrics{{grid-template-columns:1fr}}}}
</style>
</head>
<body><main class="wrap">
<div class="eyebrow">AgentContextMap · local report</div>
<h1>See what instructions your coding agents can see.</h1>
<p class="sub">Target: <strong>{target}</strong><br>Root: {root}</p>
<div class="metrics"><div class="metric"><strong>{sources}</strong><span>instruction sources</span></div><div class="metric"><strong>~{tokens}</strong><span>estimated tokens</span></div><div class="metric"><strong>{findings}</strong><span>findings</span></div></div>
<section><h2>Context map</h2><div class="toolbar"><button class="active" data-filter="all">All</button><button data-filter="codex">Codex</button><button data-filter="claude">Claude</button><button data-filter="gemini">Gemini</button><button data-filter="copilot">Copilot</button><button data-filter="cursor">Cursor</button><button data-filter="windsurf">Windsurf</button><button data-filter="cline">Cline</button></div><div class="grid">{source_cards}</div></section>
<section><h2>Findings</h2>{finding_cards}</section>
<footer>Generated locally by AgentContextMap. No repository content was sent to an external service.</footer>
</main>
<script>document.querySelectorAll('[data-filter]').forEach(b=>b.addEventListener('click',()=>{{document.querySelectorAll('[data-filter]').forEach(x=>x.classList.remove('active'));b.classList.add('active');const f=b.dataset.filter;document.querySelectorAll('.source').forEach(card=>card.hidden=f!=='all'&&card.dataset.agent!==f);}}));</script>
</body></html>"#,
        target = html_escape(&target),
        root = html_escape(&analysis.root.display().to_string()),
        sources = analysis.sources.len(),
        tokens = analysis.estimated_tokens,
        findings = analysis.findings.len(),
        source_cards = source_cards,
        finding_cards = finding_cards
    )
}

fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn target_gets_root_and_nested_agents_files() {
        let root = temp_root("nested");
        write(&root, "AGENTS.md", "Always run tests before committing.\n");
        write(&root, "src/AGENTS.md", "Use cargo test for Rust changes.\n");
        write(&root, "src/lib.rs", "pub fn demo() {}\n");

        let analysis = analyze(&root, Some(Path::new("src/lib.rs"))).unwrap();
        let paths = analysis
            .sources
            .iter()
            .map(|source| display_path(&source.path))
            .collect::<Vec<_>>();

        assert!(paths.contains(&"AGENTS.md".to_string()));
        assert!(paths.contains(&"src/AGENTS.md".to_string()));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_opposite_directives_in_overlapping_scopes() {
        let root = temp_root("conflict");
        write(&root, "AGENTS.md", "Always run tests before committing.\n");
        write(&root, "src/AGENTS.md", "Never run tests before committing.\n");
        write(&root, "src/lib.rs", "pub fn demo() {}\n");

        let analysis = analyze(&root, Some(Path::new("src/lib.rs"))).unwrap();
        assert!(analysis
            .findings
            .iter()
            .any(|finding| finding.kind == FindingKind::Contradiction));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cursor_glob_limits_target_scope() {
        let root = temp_root("cursor");
        write(
            &root,
            ".cursor/rules/rust.mdc",
            "---\nglobs: **/*.rs\nalwaysApply: false\n---\nAlways run cargo fmt.\n",
        );
        write(&root, "src/lib.rs", "pub fn demo() {}\n");
        write(&root, "README.md", "hello\n");

        let rust_analysis = analyze(&root, Some(Path::new("src/lib.rs"))).unwrap();
        assert_eq!(rust_analysis.sources.len(), 1);

        let readme_analysis = analyze(&root, Some(Path::new("README.md"))).unwrap();
        assert!(readme_analysis.sources.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_overview_discovers_multiple_agent_formats() {
        let root = temp_root("overview");
        write(&root, "AGENTS.md", "Always run tests.\n");
        write(&root, "CLAUDE.md", "Use cargo test.\n");
        write(&root, "GEMINI.md", "Use cargo test.\n");
        write(
            &root,
            ".github/copilot-instructions.md",
            "Use cargo test.\n",
        );

        let analysis = analyze(&root, None).unwrap();
        assert_eq!(analysis.sources.len(), 4);
        assert!(render_json(&analysis).contains("GitHub Copilot"));
        assert!(render_html(&analysis).contains("Context map"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wildcard_supports_common_double_star_patterns() {
        assert!(glob_list_matches("**/*.rs", Path::new("src/lib.rs")));
        assert!(glob_list_matches("**/*.rs", Path::new("lib.rs")));
        assert!(!glob_list_matches("**/*.rs", Path::new("README.md")));
    }
}
