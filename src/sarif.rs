use crate::model::{display_path, Analysis, Finding, FindingKind, Severity};
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const INFORMATION_URI: &str = "https://github.com/BLCCoreStudio/AgentContextMap";

pub fn render_sarif(analysis: &Analysis) -> String {
    let mut out = String::new();
    out.push_str("{\"$schema\":\"https://json.schemastore.org/sarif-2.1.0.json\",\"version\":\"2.1.0\",\"runs\":[{");
    out.push_str(&format!(
        "\"tool\":{{\"driver\":{{\"name\":\"AgentContextMap\",\"semanticVersion\":\"{}\",\"informationUri\":\"{}\",\"rules\":[{}]}}}},",
        json_escape(VERSION),
        json_escape(INFORMATION_URI),
        render_rules()
    ));
    out.push_str("\"results\":[");

    for (index, finding) in analysis.findings.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&render_result(analysis, finding));
    }

    out.push_str("]}]}");
    out
}

fn render_rules() -> String {
    [
        rule_json(
            "ACM001",
            "instruction-contradiction",
            "Contradictory agent instructions",
            "Overlapping instruction sources contain directives with opposite polarity.",
            "warning",
        ),
        rule_json(
            "ACM002",
            "instruction-choice-conflict",
            "Conflicting tool choice",
            "Overlapping instruction sources require incompatible choices for the same tool category.",
            "warning",
        ),
        rule_json(
            "ACM003",
            "duplicate-instruction",
            "Duplicate agent instruction",
            "The same directive appears in overlapping instruction sources.",
            "note",
        ),
        rule_json(
            "ACM004",
            "broken-instruction-reference",
            "Broken instruction reference",
            "An instruction file references repository content that could not be loaded.",
            "warning",
        ),
    ]
    .join(",")
}

fn rule_json(
    id: &str,
    name: &str,
    short_description: &str,
    full_description: &str,
    default_level: &str,
) -> String {
    format!(
        "{{\"id\":\"{}\",\"name\":\"{}\",\"shortDescription\":{{\"text\":\"{}\"}},\"fullDescription\":{{\"text\":\"{}\"}},\"defaultConfiguration\":{{\"level\":\"{}\"}},\"helpUri\":\"{}#what-it-helps-you-inspect\",\"properties\":{{\"tags\":[\"ai-agents\",\"configuration\"]}}}}",
        json_escape(id),
        json_escape(name),
        json_escape(short_description),
        json_escape(full_description),
        json_escape(default_level),
        json_escape(INFORMATION_URI)
    )
}

fn render_result(analysis: &Analysis, finding: &Finding) -> String {
    let (rule_id, rule_index) = rule_for(finding.kind);
    let left_path = display_path(&finding.left_source);
    let right_path = finding.right_source.as_deref().map(display_path);
    let message = match right_path.as_deref() {
        Some(right) => format!(
            "{} Conflicting or overlapping instruction sources: {} and {}.",
            finding.summary, left_path, right
        ),
        None => format!("{} Instruction source: {}.", finding.summary, left_path),
    };

    let mut result = format!(
        "{{\"ruleId\":\"{}\",\"ruleIndex\":{},\"level\":\"{}\",\"message\":{{\"text\":\"{}\"}},\"locations\":[{}]",
        rule_id,
        rule_index,
        sarif_level(finding.severity),
        json_escape(&message),
        render_location(
            &finding.left_source,
            find_line_number(analysis, &finding.left_source, &finding.left_line),
            None
        )
    );

    if let Some(right_source) = finding.right_source.as_deref() {
        let right_line = finding
            .right_line
            .as_deref()
            .and_then(|line| find_line_number(analysis, right_source, line));
        result.push_str(&format!(
            ",\"relatedLocations\":[{}]",
            render_location(right_source, right_line, Some(1))
        ));
    }

    result.push('}');
    result
}

fn render_location(path: &Path, line: Option<usize>, id: Option<usize>) -> String {
    let mut out = String::new();
    out.push('{');
    if let Some(id) = id {
        out.push_str(&format!("\"id\":{},", id));
    }
    out.push_str("\"physicalLocation\":{");
    out.push_str(&format!(
        "\"artifactLocation\":{{\"uri\":\"{}\"}}",
        json_escape(&display_path(path))
    ));
    if let Some(line) = line {
        out.push_str(&format!(",\"region\":{{\"startLine\":{line}}}"));
    }
    out.push_str("}}");
    out
}

fn find_line_number(analysis: &Analysis, path: &Path, needle: &str) -> Option<usize> {
    let source = analysis.sources.iter().find(|source| source.path == path)?;
    let needle = needle.trim();

    source
        .content
        .lines()
        .position(|line| {
            let trimmed = line.trim();
            trimmed == needle
                || trimmed
                    .trim_start_matches(['-', '*', '+', ' '])
                    .trim()
                    == needle
        })
        .map(|index| index + 1)
}

fn rule_for(kind: FindingKind) -> (&'static str, usize) {
    match kind {
        FindingKind::Contradiction => ("ACM001", 0),
        FindingKind::ChoiceConflict => ("ACM002", 1),
        FindingKind::Duplicate => ("ACM003", 2),
        FindingKind::BrokenReference => ("ACM004", 3),
    }
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low => "note",
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            ch if ch <= '\u{1f}' => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, InstructionSource, SourceKind};
    use std::path::PathBuf;

    fn analysis_with_conflict() -> Analysis {
        Analysis {
            root: PathBuf::from("/repo"),
            target: Some(PathBuf::from("src/lib.rs")),
            sources: vec![
                InstructionSource {
                    path: PathBuf::from("AGENTS.md"),
                    agents: vec![Agent::Codex],
                    kind: SourceKind::Workspace,
                    scope: PathBuf::new(),
                    patterns: Vec::new(),
                    bytes: 40,
                    content: "# Rules\n- Always run tests before committing.\n".to_string(),
                    imports: Vec::new(),
                    notes: Vec::new(),
                },
                InstructionSource {
                    path: PathBuf::from("src/AGENTS.md"),
                    agents: vec![Agent::Codex],
                    kind: SourceKind::Workspace,
                    scope: PathBuf::new(),
                    patterns: Vec::new(),
                    bytes: 39,
                    content: "Never run tests before committing.\n".to_string(),
                    imports: Vec::new(),
                    notes: Vec::new(),
                },
            ],
            findings: vec![Finding {
                kind: FindingKind::Contradiction,
                severity: Severity::High,
                left_source: PathBuf::from("AGENTS.md"),
                right_source: Some(PathBuf::from("src/AGENTS.md")),
                left_line: "Always run tests before committing.".to_string(),
                right_line: Some("Never run tests before committing.".to_string()),
                summary: "Overlapping sources contain directives with opposite polarity."
                    .to_string(),
            }],
            total_bytes: 79,
            estimated_tokens: 20,
        }
    }

    #[test]
    fn emits_github_compatible_sarif_shape() {
        let sarif = render_sarif(&analysis_with_conflict());
        assert!(sarif.contains("\"version\":\"2.1.0\""));
        assert!(sarif.contains("\"ruleId\":\"ACM001\""));
        assert!(sarif.contains("\"level\":\"error\""));
        assert!(sarif.contains("\"uri\":\"AGENTS.md\""));
        assert!(sarif.contains("\"startLine\":2"));
        assert!(sarif.contains("\"relatedLocations\""));
        assert!(sarif.contains("\"uri\":\"src/AGENTS.md\""));
    }

    #[test]
    fn escapes_json_control_characters() {
        assert_eq!(json_escape("a\n\"b\\c\u{0001}"), "a\\n\\\"b\\\\c\\u0001");
    }
}
