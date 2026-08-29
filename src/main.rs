use agentcontextmap::{analyze, render_html, render_json, render_text, FindingKind};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return;
    }
    if args.iter().any(|arg| arg == "-V" || arg == "--version") {
        println!("agentcontext {VERSION}");
        return;
    }

    let mut root = PathBuf::from(".");
    let mut target = None;
    let mut json = false;
    let mut html = None;
    let mut fail_on_conflict = false;
    let mut positional_seen = false;

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--target" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    die("--target requires a path");
                };
                target = Some(PathBuf::from(value));
            }
            "--json" => json = true,
            "--html" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    die("--html requires an output path");
                };
                html = Some(PathBuf::from(value));
            }
            "--fail-on-conflict" => fail_on_conflict = true,
            value if value.starts_with('-') => die(&format!("unknown option: {value}")),
            value => {
                if positional_seen {
                    die("only one repository root can be supplied");
                }
                root = PathBuf::from(value);
                positional_seen = true;
            }
        }
        index += 1;
    }

    let analysis = match analyze(&root, target.as_deref()) {
        Ok(analysis) => analysis,
        Err(error) => die(&format!("failed to analyze {}: {error}", root.display())),
    };

    if let Some(output) = html {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(error) = fs::create_dir_all(parent) {
                    die(&format!("failed to create {}: {error}", parent.display()));
                }
            }
        }
        if let Err(error) = fs::write(&output, render_html(&analysis)) {
            die(&format!("failed to write {}: {error}", output.display()));
        }
        if !json {
            eprintln!("HTML report: {}", output.display());
        }
    }

    if json {
        println!("{}", render_json(&analysis));
    } else {
        print!("{}", render_text(&analysis));
    }

    if fail_on_conflict
        && analysis.findings.iter().any(|finding| {
            matches!(
                finding.kind,
                FindingKind::Contradiction | FindingKind::ChoiceConflict
            )
        })
    {
        process::exit(2);
    }
}

fn die(message: &str) -> ! {
    eprintln!("agentcontext: {message}");
    process::exit(1);
}

fn print_help() {
    println!(
        "AgentContextMap {VERSION}\n\n\
See which repository instructions can affect your coding agents.\n\n\
USAGE:\n    agentcontext [ROOT] [OPTIONS]\n\n\
ARGUMENTS:\n    [ROOT]                 Repository root to inspect [default: .]\n\n\
OPTIONS:\n    --target <PATH>        Show the effective instruction chain for a target path\n    --json                 Emit machine-readable JSON\n    --html <PATH>          Write a self-contained visual HTML report\n    --fail-on-conflict     Exit with code 2 when a high-severity conflict is detected\n    -h, --help             Print help\n    -V, --version          Print version\n\n\
EXAMPLES:\n    agentcontext .\n    agentcontext . --target src/api/auth.rs\n    agentcontext . --target src/api/auth.rs --html report.html\n    agentcontext . --json --fail-on-conflict"
    );
}
