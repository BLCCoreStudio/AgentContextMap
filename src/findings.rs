use crate::model::{ActivationState, Finding, FindingKind, InstructionSource, Severity, SourceKind};
use std::collections::{HashSet, BTreeSet};
use std::path::Path;

#[derive(Debug)]
struct Directive {
    source_index: usize,
    line: String,
    normalized: String,
    polarity: i8,
    keywords: BTreeSet<String>,
    choice: Option<&'static str>,
}

pub fn detect_findings(sources: &[InstructionSource], target: Option<&Path>) -> Vec<Finding> {
    let directives = collect_directives(sources);
    let mut findings = Vec::new();
    let mut seen = HashSet::new();

    for (source_index, source) in sources.iter().enumerate() {
        for import in &source.imports {
            if matches!(import.status, crate::model::ImportStatus::Missing) {
                let key = format!("ref:{source_index}:{}", crate::model::display_path(&import.path));
                if seen.insert(key) {
                    findings.push(Finding {
                        kind: FindingKind::BrokenReference,
                        severity: Severity::Medium,
                        left_source: source.path.clone(),
                        right_source: None,
                        left_line: format!("@{}", crate::model::display_path(&import.path)),
                        right_line: None,
                        summary: "Referenced instruction import could not be loaded from the repository.".to_string(),
                    });
                }
            }
        }
    }

    for left_index in 0..directives.len() {
        for right_index in (left_index + 1)..directives.len() {
            let left = &directives[left_index];
            let right = &directives[right_index];
            if left.source_index == right.source_index {
                continue;
            }
            let left_source = &sources[left.source_index];
            let right_source = &sources[right.source_index];
            if !sources_can_overlap(left_source, right_source, target) {
                continue;
            }

            if left.normalized == right.normalized {
                let key = canonical_key("duplicate", left_source, right_source, &left.normalized);
                if seen.insert(key) {
                    findings.push(Finding {
                        kind: FindingKind::Duplicate,
                        severity: Severity::Low,
                        left_source: left_source.path.clone(),
                        right_source: Some(right_source.path.clone()),
                        left_line: left.line.clone(),
                        right_line: Some(right.line.clone()),
                        summary: "The same directive appears in overlapping instruction sources.".to_string(),
                    });
                }
                continue;
            }

            if let (Some(left_choice), Some(right_choice)) = (left.choice, right.choice) {
                if left_choice != right_choice {
                    let key = canonical_key("choice", left_source, right_source, "package-manager");
                    if seen.insert(key) {
                        findings.push(Finding {
                            kind: FindingKind::ChoiceConflict,
                            severity: conflict_severity(left_source, right_source, target),
                            left_source: left_source.path.clone(),
                            right_source: Some(right_source.path.clone()),
                            left_line: left.line.clone(),
                            right_line: Some(right.line.clone()),
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
                && keyword_overlap(&left.keywords, &right.keywords) >= 0.55
            {
                let shared = left
                    .keywords
                    .intersection(&right.keywords)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",");
                let key = canonical_key("conflict", left_source, right_source, &shared);
                if seen.insert(key) {
                    findings.push(Finding {
                        kind: FindingKind::Contradiction,
                        severity: conflict_severity(left_source, right_source, target),
                        left_source: left_source.path.clone(),
                        right_source: Some(right_source.path.clone()),
                        left_line: left.line.clone(),
                        right_line: Some(right.line.clone()),
                        summary: "Overlapping sources contain directives with opposite polarity.".to_string(),
                    });
                }
            }
        }
    }

    findings.sort_by_key(|finding| match finding.severity {
        Severity::High => 0,
        Severity::Medium => 1,
        Severity::Low => 2,
    });
    findings
}

fn collect_directives(sources: &[InstructionSource]) -> Vec<Directive> {
    let mut directives = Vec::new();
    for (source_index, source) in sources.iter().enumerate() {
        let mut in_fence = false;
        for raw_line in source.content.lines() {
            let trimmed = raw_line.trim();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence || trimmed.is_empty() || trimmed == "---" || trimmed.starts_with('#') {
                continue;
            }
            let line = trimmed
                .trim_start_matches(|ch: char| matches!(ch, '-' | '*' | '+' | ' '))
                .trim();
            if line.len() < 5 || line.len() > 220 || line.contains("AgentContextMap:") {
                continue;
            }
            let normalized = normalize(line);
            if normalized.len() < 4 {
                continue;
            }
            directives.push(Directive {
                source_index,
                line: line.to_string(),
                polarity: polarity(&normalized),
                keywords: keyword_set(&normalized),
                choice: package_manager(&normalized),
                normalized,
            });
        }
    }
    directives
}

fn sources_can_overlap(left: &InstructionSource, right: &InstructionSource, target: Option<&Path>) -> bool {
    if !left.agents.iter().any(|agent| right.agents.contains(agent)) {
        return false;
    }

    if target.is_some() {
        return true;
    }

    if matches!(left.kind, SourceKind::Pattern) || matches!(right.kind, SourceKind::Pattern) {
        return false;
    }

    match (left.kind, right.kind) {
        (SourceKind::Hierarchical, SourceKind::Hierarchical) => {
            left.scope.starts_with(&right.scope) || right.scope.starts_with(&left.scope)
        }
        _ => true,
    }
}

fn conflict_severity(left: &InstructionSource, right: &InstructionSource, target: Option<&Path>) -> Severity {
    let left_state = left.activation_state(target);
    let right_state = right.activation_state(target);
    if left_state.definite_for_target() && right_state.definite_for_target() {
        Severity::High
    } else if matches!(left_state, ActivationState::Manual) || matches!(right_state, ActivationState::Manual) {
        Severity::Low
    } else {
        Severity::Medium
    }
}

fn canonical_key(kind: &str, left: &InstructionSource, right: &InstructionSource, detail: &str) -> String {
    let left_path = crate::model::display_path(&left.path);
    let right_path = crate::model::display_path(&right.path);
    if left_path <= right_path {
        format!("{kind}:{left_path}:{right_path}:{detail}")
    } else {
        format!("{kind}:{right_path}:{left_path}:{detail}")
    }
}

fn normalize(line: &str) -> String {
    line.to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() || ch == '-' { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn polarity(line: &str) -> i8 {
    let negative_phrases = ["never ", "do not ", "don't ", "must not ", "avoid ", "forbid ", "forbidden "];
    if negative_phrases.iter().any(|phrase| line.starts_with(phrase) || line.contains(&format!(" {phrase}"))) {
        return -1;
    }
    let positive_words = ["always", "must", "required", "require", "use", "run", "prefer", "should"];
    if positive_words.iter().any(|word| contains_word(line, word)) {
        return 1;
    }
    0
}

fn package_manager(line: &str) -> Option<&'static str> {
    ["pnpm", "yarn", "npm", "bun"]
        .into_iter()
        .find(|candidate| contains_word(line, candidate))
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack.split_whitespace().any(|word| word == needle)
}

fn keyword_set(line: &str) -> BTreeSet<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "before", "by", "do", "dont", "for", "from",
        "in", "is", "it", "must", "never", "not", "of", "on", "or", "should", "the", "this", "to",
        "use", "always", "required", "require", "without", "with", "avoid", "prefer", "run",
    ];
    line.split_whitespace()
        .filter(|word| word.len() > 2 && !STOPWORDS.contains(word))
        .map(ToString::to_string)
        .collect()
}

fn keyword_overlap(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f32 {
    let denominator = left.len().min(right.len());
    if denominator == 0 {
        return 0.0;
    }
    left.intersection(right).count() as f32 / denominator as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, InstructionSource};
    use std::path::PathBuf;

    fn source(path: &str, agent: Agent, content: &str) -> InstructionSource {
        InstructionSource {
            path: PathBuf::from(path),
            agents: vec![agent],
            kind: SourceKind::Workspace,
            scope: PathBuf::new(),
            patterns: Vec::new(),
            bytes: content.len(),
            content: content.to_string(),
            imports: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn does_not_report_cross_agent_conflicts() {
        let sources = vec![
            source("CLAUDE.md", Agent::Claude, "Always run tests before committing."),
            source("GEMINI.md", Agent::Gemini, "Never run tests before committing."),
        ];
        assert!(detect_findings(&sources, Some(Path::new("src/lib.rs"))).is_empty());
    }

    #[test]
    fn reports_same_agent_conflict_as_high_for_target() {
        let sources = vec![
            source("one.md", Agent::Codex, "Always run tests before committing."),
            source("two.md", Agent::Codex, "Never run tests before committing."),
        ];
        let findings = detect_findings(&sources, Some(Path::new("src/lib.rs")));
        assert!(findings.iter().any(|finding| finding.kind == FindingKind::Contradiction && finding.severity == Severity::High));
    }
}
