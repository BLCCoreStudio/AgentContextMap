use crate::model::{display_path, Analysis, ImportStatus, Severity};

pub fn render_text(analysis: &Analysis) -> String {
    let mut out = String::new();
    out.push_str("AgentContextMap\n===============\n");
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

    out.push_str("Instruction sources\n-------------------\n");
    for (index, source) in analysis.sources.iter().enumerate() {
        let state = source.activation_state(analysis.target.as_deref());
        out.push_str(&format!(
            "{}. {}\n   Agents: {}\n   Status: {} | Scope: {}\n",
            index + 1,
            display_path(&source.path),
            source.agent_labels(),
            state.label(),
            source.scope_label()
        ));
        if !source.imports.is_empty() {
            out.push_str("   Imports:\n");
            for import in &source.imports {
                out.push_str(&format!(
                    "   - {} [{}]\n",
                    display_path(&import.path),
                    import.status.label()
                ));
            }
        }
        for note in &source.notes {
            out.push_str(&format!("   Note: {note}\n"));
        }
    }

    out.push_str("\nFindings\n--------\n");
    if analysis.findings.is_empty() {
        out.push_str("None detected by the current deterministic checks.\n");
    } else {
        for finding in &analysis.findings {
            out.push_str(&format!(
                "{} [{}] {}",
                finding.kind.label(),
                finding.severity.label(),
                display_path(&finding.left_source)
            ));
            if let Some(right) = &finding.right_source {
                out.push_str(&format!(" <-> {}", display_path(right)));
            }
            out.push('\n');
            out.push_str(&format!("  {}\n", finding.summary));
            out.push_str(&format!("  - {}\n", finding.left_line));
            if let Some(right_line) = &finding.right_line {
                out.push_str(&format!("  - {right_line}\n"));
            }
        }
    }
    out
}

pub fn render_json(analysis: &Analysis) -> String {
    let mut json = String::new();
    json.push('{');
    json.push_str(&format!(
        "\"root\":\"{}\",",
        json_escape(&analysis.root.display().to_string())
    ));
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
        let agents = source
            .agents
            .iter()
            .map(|agent| format!("\"{}\"", json_escape(agent.label())))
            .collect::<Vec<_>>()
            .join(",");
        let patterns = source
            .patterns
            .iter()
            .map(|pattern| format!("\"{}\"", json_escape(pattern)))
            .collect::<Vec<_>>()
            .join(",");
        let imports = source
            .imports
            .iter()
            .map(|import| {
                format!(
                    "{{\"path\":\"{}\",\"status\":\"{}\",\"bytes\":{}}}",
                    json_escape(&display_path(&import.path)),
                    import.status.label(),
                    import.bytes
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let notes = source
            .notes
            .iter()
            .map(|note| format!("\"{}\"", json_escape(note)))
            .collect::<Vec<_>>()
            .join(",");
        json.push_str(&format!(
            "{{\"path\":\"{}\",\"agents\":[{}],\"status\":\"{}\",\"scope\":\"{}\",\"patterns\":[{}],\"bytes\":{},\"imports\":[{}],\"notes\":[{}]}}",
            json_escape(&display_path(&source.path)),
            agents,
            source.activation_state(analysis.target.as_deref()).label(),
            json_escape(&source.scope_label()),
            patterns,
            source.bytes,
            imports,
            notes
        ));
    }

    json.push_str("],\"findings\":[");
    for (index, finding) in analysis.findings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let right_source = finding
            .right_source
            .as_ref()
            .map(|path| format!("\"{}\"", json_escape(&display_path(path))))
            .unwrap_or_else(|| "null".to_string());
        let right_line = finding
            .right_line
            .as_ref()
            .map(|line| format!("\"{}\"", json_escape(line)))
            .unwrap_or_else(|| "null".to_string());
        json.push_str(&format!(
            "{{\"kind\":\"{}\",\"severity\":\"{}\",\"left_source\":\"{}\",\"right_source\":{},\"left_line\":\"{}\",\"right_line\":{},\"summary\":\"{}\"}}",
            finding.kind.label(),
            finding.severity.label(),
            json_escape(&display_path(&finding.left_source)),
            right_source,
            json_escape(&finding.left_line),
            right_line,
            json_escape(&finding.summary)
        ));
    }
    json.push_str("]}");
    json
}

pub fn render_html(analysis: &Analysis) -> String {
    let target = analysis
        .target
        .as_deref()
        .map(display_path)
        .unwrap_or_else(|| "workspace overview".to_string());
    let high_findings = analysis
        .findings
        .iter()
        .filter(|finding| finding.severity == Severity::High)
        .count();

    let mut out = String::new();
    out.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>AgentContextMap report</title>");
    out.push_str(r#"<style>
:root{--bg:#0b1020;--panel:#121a2f;--panel2:#18223b;--text:#eef3ff;--muted:#9eabc7;--line:#2a3655;--accent:#8ab4ff;--warn:#ffd580;--bad:#ff8f9c;--good:#7ee2b8}
*{box-sizing:border-box}body{margin:0;background:linear-gradient(180deg,#0b1020,#0d1324 45%,#090d18);color:var(--text);font:15px/1.5 ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}.report{max-width:1180px;margin:auto;padding:44px 22px 72px}.eyebrow{color:var(--accent);font-weight:700;letter-spacing:.08em;text-transform:uppercase;font-size:12px}h1{font-size:clamp(34px,6vw,62px);line-height:1;margin:10px 0 14px;letter-spacing:-.04em}.sub{color:var(--muted);font-size:17px;max-width:850px}.notice{margin:20px 0;padding:12px 14px;border:1px solid var(--line);border-radius:12px;color:var(--muted);background:rgba(18,26,47,.55)}
.metrics{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin:28px 0}.metric{background:rgba(18,26,47,.8);border:1px solid var(--line);border-radius:16px;padding:18px}.metric strong{font-size:27px;display:block}.metric span{color:var(--muted)}section{margin-top:42px}h2{font-size:24px;margin-bottom:14px}
.controls{display:grid;grid-template-columns:minmax(180px,1fr) auto;gap:12px;align-items:start;margin:18px 0}.search,.status{border:1px solid var(--line);background:var(--panel);color:var(--text);border-radius:11px;padding:11px 12px;font:inherit}.toolbar{display:flex;flex-wrap:wrap;gap:8px;margin:8px 0 18px}.filter,.clear-highlight,.finding-jump{border:1px solid var(--line);background:var(--panel);color:var(--text);padding:9px 13px;border-radius:999px;cursor:pointer}.filter:hover,.filter.active,.finding-jump:hover,.clear-highlight:hover{border-color:var(--accent);color:var(--accent)}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(285px,1fr));gap:12px}.source{background:var(--panel);border:1px solid var(--line);border-radius:16px;min-width:0;overflow:hidden}.source[hidden]{display:none}.source.highlight{border-color:var(--bad);box-shadow:0 0 0 1px var(--bad) inset}.source-toggle{width:100%;text-align:left;border:0;background:transparent;color:inherit;padding:17px;cursor:pointer}.source-toggle:focus-visible{outline:2px solid var(--accent);outline-offset:-3px}.source-top{display:flex;justify-content:space-between;gap:12px}.agent{color:var(--accent);font-weight:700}.state{font-size:11px;text-transform:uppercase;color:var(--muted)}.source h3{margin:16px 0 6px;font-size:16px;word-break:break-word}.meta{margin:0;color:var(--muted)}.hint{display:block;margin-top:12px;color:var(--accent);font-size:12px}.source-details{border-top:1px solid var(--line);padding:15px 17px;background:var(--panel2)}.source-details h4{margin:0 0 8px}.source-details pre{max-height:320px;overflow:auto;white-space:pre-wrap;word-break:break-word;background:#0d1428;border:1px solid var(--line);border-radius:10px;padding:12px;color:#d9e2f7}.source-details ul{padding-left:20px;color:var(--muted)}
.finding{background:var(--panel);border:1px solid var(--line);border-left:4px solid var(--bad);border-radius:14px;padding:16px;margin:10px 0}.finding.medium{border-left-color:var(--warn)}.finding.low{border-left-color:var(--good)}.finding-head{display:flex;align-items:center;justify-content:space-between;gap:12px}.finding-head span{color:var(--muted);text-transform:uppercase;font-size:11px}.finding code{display:block;background:var(--panel2);padding:10px;border-radius:9px;margin:7px 0;white-space:pre-wrap;color:#d9e2f7}.finding small{color:var(--muted)}.empty{color:var(--muted);border:1px dashed var(--line);padding:18px;border-radius:14px}.no-results{display:none;color:var(--muted);padding:18px;border:1px dashed var(--line);border-radius:14px}.no-results.show{display:block}footer{margin-top:52px;color:var(--muted);font-size:13px}
@media(max-width:760px){.metrics{grid-template-columns:1fr 1fr}.controls{grid-template-columns:1fr}}@media(max-width:480px){.metrics{grid-template-columns:1fr}}
</style>"#);
    out.push_str("</head><body><main class=\"report\">");
    out.push_str("<div class=\"eyebrow\">AgentContextMap · local report</div><h1>See which instructions can affect your coding agents.</h1>");
    out.push_str(&format!(
        "<p class=\"sub\">Target: <strong>{}</strong><br>Root: {}</p>",
        html_escape(&target),
        html_escape(&analysis.root.display().to_string())
    ));
    out.push_str("<div class=\"notice\"><strong>This page is a report viewer.</strong> The CLI already performed the filesystem analysis. Filters, source details and finding highlights work here; changing files on disk requires running <code>agentcontext</code> again.</div>");
    out.push_str(&format!(
        "<div class=\"metrics\"><div class=\"metric\"><strong>{}</strong><span>instruction sources</span></div><div class=\"metric\"><strong>~{}</strong><span>estimated tokens</span></div><div class=\"metric\"><strong>{}</strong><span>findings</span></div><div class=\"metric\"><strong>{}</strong><span>high severity</span></div></div>",
        analysis.sources.len(), analysis.estimated_tokens, analysis.findings.len(), high_findings
    ));

    out.push_str("<section><h2>Context map</h2><div class=\"controls\"><input id=\"sourceSearch\" class=\"search\" type=\"search\" placeholder=\"Filter by path, agent or content…\" aria-label=\"Filter sources\"><select id=\"statusFilter\" class=\"status\" aria-label=\"Filter by activation status\"><option value=\"all\">All activation states</option><option value=\"active\">Active</option><option value=\"directory\">Directory-scoped</option><option value=\"path\">Path-specific</option><option value=\"conditional\">Conditional</option><option value=\"manual\">Manual</option></select></div>");
    out.push_str("<div class=\"toolbar\"><button class=\"filter active\" type=\"button\" data-agent=\"all\">All</button><button class=\"filter\" type=\"button\" data-agent=\"codex\">Codex</button><button class=\"filter\" type=\"button\" data-agent=\"claude\">Claude</button><button class=\"filter\" type=\"button\" data-agent=\"gemini\">Gemini</button><button class=\"filter\" type=\"button\" data-agent=\"copilot\">Copilot</button><button class=\"filter\" type=\"button\" data-agent=\"cursor\">Cursor</button><button class=\"filter\" type=\"button\" data-agent=\"windsurf\">Windsurf</button><button class=\"filter\" type=\"button\" data-agent=\"cline\">Cline</button><button class=\"clear-highlight\" type=\"button\">Clear highlight</button></div><div class=\"grid\" id=\"sourceGrid\">");

    for (index, source) in analysis.sources.iter().enumerate() {
        let agent_slugs = source
            .agents
            .iter()
            .map(|agent| agent.slug())
            .collect::<Vec<_>>()
            .join(" ");
        let agents = source.agent_labels();
        let state = source.activation_state(analysis.target.as_deref());
        let path = display_path(&source.path);
        out.push_str(&format!(
            "<article class=\"source\" data-path=\"{}\" data-agents=\"{}\" data-status=\"{}\"><button class=\"source-toggle\" type=\"button\" aria-expanded=\"false\" aria-controls=\"source-detail-{}\"><div class=\"source-top\"><span class=\"agent\">{}</span><span class=\"state\">{}</span></div><h3>{}</h3><p class=\"meta\">{} · {} bytes</p><span class=\"hint\">View source details</span></button><div class=\"source-details\" id=\"source-detail-{}\" hidden>",
            html_escape(&path),
            html_escape(&agent_slugs),
            state.slug(),
            index,
            html_escape(&agents),
            state.label(),
            html_escape(&path),
            html_escape(&source.scope_label()),
            source.bytes,
            index
        ));
        if !source.patterns.is_empty() {
            out.push_str(&format!(
                "<p><strong>Patterns:</strong> {}</p>",
                html_escape(&source.patterns.join(", "))
            ));
        }
        if !source.notes.is_empty() {
            out.push_str("<h4>Notes</h4><ul>");
            for note in &source.notes {
                out.push_str(&format!("<li>{}</li>", html_escape(note)));
            }
            out.push_str("</ul>");
        }
        if !source.imports.is_empty() {
            out.push_str("<h4>Imports</h4><ul>");
            for import in &source.imports {
                let suffix = match import.status {
                    ImportStatus::Loaded => format!("{} bytes", import.bytes),
                    _ => import.status.label().to_string(),
                };
                out.push_str(&format!(
                    "<li>{} — {}</li>",
                    html_escape(&display_path(&import.path)),
                    html_escape(&suffix)
                ));
            }
            out.push_str("</ul>");
        }
        out.push_str(&format!(
            "<h4>Instruction text</h4><pre>{}</pre></div></article>",
            html_escape(&source.content)
        ));
    }
    out.push_str("</div><div class=\"no-results\" id=\"noResults\">No instruction sources match the current filters.</div></section>");

    out.push_str("<section><h2>Findings</h2>");
    if analysis.findings.is_empty() {
        out.push_str("<div class=\"empty\">No conflicts, duplicates or broken repository imports detected by the current deterministic checks.</div>");
    } else {
        for finding in &analysis.findings {
            let left = display_path(&finding.left_source);
            let right = finding
                .right_source
                .as_ref()
                .map(|path| display_path(path))
                .unwrap_or_default();
            out.push_str(&format!(
                "<article class=\"finding {}\"><div class=\"finding-head\"><strong>{}</strong><span>{}</span></div><p>{}</p><code>{}</code>",
                finding.severity.label(),
                finding.kind.label(),
                finding.severity.label(),
                html_escape(&finding.summary),
                html_escape(&finding.left_line)
            ));
            if let Some(right_line) = &finding.right_line {
                out.push_str(&format!("<code>{}</code>", html_escape(right_line)));
            }
            if !right.is_empty() {
                out.push_str(&format!("<small>{} ↔ {}</small><br><button class=\"finding-jump\" type=\"button\" data-left=\"{}\" data-right=\"{}\">Highlight involved sources</button>", html_escape(&left), html_escape(&right), html_escape(&left), html_escape(&right)));
            } else {
                out.push_str(&format!("<small>{}</small><br><button class=\"finding-jump\" type=\"button\" data-left=\"{}\" data-right=\"\">Highlight source</button>", html_escape(&left), html_escape(&left)));
            }
            out.push_str("</article>");
        }
    }
    out.push_str("</section><footer>Generated locally by AgentContextMap. No repository content was sent to an external service.</footer></main>");
    out.push_str(r#"<script>
(() => {
  const report = document.querySelector('.report');
  if (!report || report.dataset.ready === '1') return;
  report.dataset.ready = '1';
  const cards = [...report.querySelectorAll('.source')];
  const search = report.querySelector('#sourceSearch');
  const status = report.querySelector('#statusFilter');
  const noResults = report.querySelector('#noResults');
  let selectedAgent = 'all';

  function applyFilters() {
    const query = (search.value || '').trim().toLowerCase();
    let visible = 0;
    cards.forEach(card => {
      const agentMatch = selectedAgent === 'all' || card.dataset.agents.split(' ').includes(selectedAgent);
      const statusMatch = status.value === 'all' || card.dataset.status === status.value;
      const textMatch = !query || card.textContent.toLowerCase().includes(query);
      card.hidden = !(agentMatch && statusMatch && textMatch);
      if (!card.hidden) visible += 1;
    });
    noResults.classList.toggle('show', visible === 0);
  }

  report.querySelectorAll('.filter').forEach(button => {
    button.addEventListener('click', () => {
      report.querySelectorAll('.filter').forEach(item => item.classList.remove('active'));
      button.classList.add('active');
      selectedAgent = button.dataset.agent;
      applyFilters();
    });
  });
  search.addEventListener('input', applyFilters);
  status.addEventListener('change', applyFilters);

  report.querySelectorAll('.source-toggle').forEach(button => {
    button.addEventListener('click', () => {
      const details = document.getElementById(button.getAttribute('aria-controls'));
      const open = button.getAttribute('aria-expanded') === 'true';
      button.setAttribute('aria-expanded', String(!open));
      details.hidden = open;
      const hint = button.querySelector('.hint');
      if (hint) hint.textContent = open ? 'View source details' : 'Hide source details';
    });
  });

  function clearHighlight() {
    cards.forEach(card => card.classList.remove('highlight'));
  }
  report.querySelector('.clear-highlight').addEventListener('click', clearHighlight);
  report.querySelectorAll('.finding-jump').forEach(button => {
    button.addEventListener('click', () => {
      clearHighlight();
      const wanted = [button.dataset.left, button.dataset.right].filter(Boolean);
      const matches = cards.filter(card => wanted.includes(card.dataset.path));
      matches.forEach(card => {
        card.hidden = false;
        card.classList.add('highlight');
      });
      if (matches[0]) matches[0].scrollIntoView({behavior:'smooth', block:'center'});
    });
  });
})();
</script></body></html>"#);
    out
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
    use crate::model::{Agent, Analysis, InstructionSource, SourceKind};
    use std::path::PathBuf;

    #[test]
    fn html_contains_real_controls_and_expandable_details() {
        let analysis = Analysis {
            root: PathBuf::from("/repo"),
            target: Some(PathBuf::from("src/lib.rs")),
            sources: vec![InstructionSource {
                path: PathBuf::from("AGENTS.md"),
                agents: vec![Agent::Codex],
                kind: SourceKind::Hierarchical,
                scope: PathBuf::new(),
                patterns: Vec::new(),
                bytes: 10,
                content: "Always test.".to_string(),
                imports: Vec::new(),
                notes: Vec::new(),
            }],
            findings: Vec::new(),
            total_bytes: 10,
            estimated_tokens: 3,
        };
        let html = render_html(&analysis);
        assert!(html.contains("sourceSearch"));
        assert!(html.contains("source-toggle"));
        assert!(html.contains("View source details"));
        assert!(html.contains("This page is a report viewer"));
    }
}
