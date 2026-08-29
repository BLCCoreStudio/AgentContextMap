use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
            Agent::Codex => "Codex",
            Agent::Claude => "Claude Code",
            Agent::Gemini => "Gemini CLI",
            Agent::Copilot => "GitHub Copilot",
            Agent::Cursor => "Cursor",
            Agent::Windsurf => "Windsurf",
            Agent::Cline => "Cline",
        }
    }

    pub fn slug(self) -> &'static str {
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
    ModelDecision,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationState {
    Active,
    DirectoryScoped,
    PathSpecific,
    Conditional,
    Manual,
}

impl ActivationState {
    pub fn label(self) -> &'static str {
        match self {
            ActivationState::Active => "active",
            ActivationState::DirectoryScoped => "directory-scoped",
            ActivationState::PathSpecific => "path-specific",
            ActivationState::Conditional => "conditional",
            ActivationState::Manual => "manual",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ActivationState::Active => "active",
            ActivationState::DirectoryScoped => "directory",
            ActivationState::PathSpecific => "path",
            ActivationState::Conditional => "conditional",
            ActivationState::Manual => "manual",
        }
    }

    pub fn definite_for_target(self) -> bool {
        matches!(self, ActivationState::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportStatus {
    Loaded,
    Missing,
    OutsideRepository,
    DepthLimit,
}

impl ImportStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ImportStatus::Loaded => "loaded",
            ImportStatus::Missing => "missing",
            ImportStatus::OutsideRepository => "outside-repository",
            ImportStatus::DepthLimit => "depth-limit",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportRef {
    pub path: PathBuf,
    pub status: ImportStatus,
    pub bytes: usize,
}

#[derive(Debug, Clone)]
pub struct InstructionSource {
    pub path: PathBuf,
    pub agents: Vec<Agent>,
    pub kind: SourceKind,
    pub scope: PathBuf,
    pub patterns: Vec<String>,
    pub bytes: usize,
    pub content: String,
    pub imports: Vec<ImportRef>,
    pub notes: Vec<String>,
}

impl InstructionSource {
    pub fn applies_to(&self, target: &Path) -> bool {
        match self.kind {
            SourceKind::Hierarchical => target.starts_with(&self.scope),
            SourceKind::Workspace => true,
            SourceKind::Pattern => self
                .patterns
                .iter()
                .any(|pattern| crate::discovery::glob_matches(pattern, target)),
            SourceKind::ModelDecision | SourceKind::Manual => true,
        }
    }

    pub fn depth(&self) -> usize {
        self.scope.components().count()
    }

    pub fn scope_label(&self) -> String {
        match self.kind {
            SourceKind::Hierarchical => {
                if self.scope.as_os_str().is_empty() {
                    "workspace tree".to_string()
                } else {
                    format!("{} subtree", display_path(&self.scope))
                }
            }
            SourceKind::Workspace => "workspace-wide".to_string(),
            SourceKind::Pattern => {
                if self.patterns.is_empty() {
                    "path-specific".to_string()
                } else {
                    format!("pattern: {}", self.patterns.join(", "))
                }
            }
            SourceKind::ModelDecision => "agent/model decision".to_string(),
            SourceKind::Manual => "manual activation".to_string(),
        }
    }

    pub fn activation_state(&self, target: Option<&Path>) -> ActivationState {
        match self.kind {
            SourceKind::Workspace => ActivationState::Active,
            SourceKind::Hierarchical => {
                if target.is_some() {
                    ActivationState::Active
                } else {
                    ActivationState::DirectoryScoped
                }
            }
            SourceKind::Pattern => {
                if target.is_some() {
                    ActivationState::Active
                } else {
                    ActivationState::PathSpecific
                }
            }
            SourceKind::ModelDecision => ActivationState::Conditional,
            SourceKind::Manual => ActivationState::Manual,
        }
    }

    pub fn agent_labels(&self) -> String {
        self.agents
            .iter()
            .map(|agent| agent.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    Contradiction,
    ChoiceConflict,
    Duplicate,
    BrokenReference,
}

impl FindingKind {
    pub fn label(self) -> &'static str {
        match self {
            FindingKind::Contradiction => "CONFLICT",
            FindingKind::ChoiceConflict => "CHOICE",
            FindingKind::Duplicate => "DUPLICATE",
            FindingKind::BrokenReference => "REFERENCE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub left_source: PathBuf,
    pub right_source: Option<PathBuf>,
    pub left_line: String,
    pub right_line: Option<String>,
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

pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}
