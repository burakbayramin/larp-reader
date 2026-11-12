//! The fake shell shown by the hide key in `shell` style (`shell_prompt` / `shell_output` in `ui/app.py`).

use std::path::{Path, PathBuf};

use crate::masks::base::{Line, plain_line, seg};
use crate::masks::{Mask, Rng, randint};

const LISTING: &[(&str, &str)] = &[
    ("README.md", "#d4d4d4"),
    ("docs", "bold #58a6ff"),
    ("pyproject.toml", "#d4d4d4"),
    ("src", "bold #58a6ff"),
    ("tests", "bold #58a6ff"),
];
const GIT_STATUS: &[&str] = &[
    "On branch main",
    "Your branch is up to date with 'origin/main'.",
    "",
    "nothing to commit, working tree clean",
];
/// Commands that answer with a burst of the current mask's output.
const TOOLS: &[&str] = &[
    "make",
    "cmake",
    "npm",
    "npx",
    "pnpm",
    "yarn",
    "node",
    "tsc",
    "python",
    "python3",
    "py",
    "pip",
    "pip3",
    "pytest",
    "uv",
    "poetry",
    "cargo",
    "rustc",
    "go",
    "dotnet",
    "mvn",
    "gradle",
    "./gradlew",
    "docker",
    "kubectl",
    "terraform",
    "ruff",
    "mypy",
    "black",
    "eslint",
    "git",
];

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
}

fn host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            let out = std::process::Command::new("hostname").output().ok()?;
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .map(|h| h.split('.').next().unwrap_or("").to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "localhost".into())
}

/// A prompt like the user's own shell would show for the current folder.
pub fn shell_prompt() -> String {
    let dir = cwd();
    if cfg!(windows) {
        return format!("PS {}> ", dir.display());
    }
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "dev".into());
    let host = host();
    if std::env::var("SHELL").unwrap_or_default().ends_with("zsh") {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into());
        return format!("{user}@{host} {name} % ");
    }
    let shown = match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) if dir.starts_with(&home) => format!(
            "~{}",
            dir.strip_prefix(&home)
                .map(|r| format!("/{}", r.display()))
                .unwrap_or_default()
        )
        .trim_end_matches('/')
        .to_string(),
        _ => dir.display().to_string(),
    };
    format!("{user}@{host}:{shown}$ ")
}

/// What a shell prints for `command`; None means clear the screen.
pub fn shell_output(
    command: &str,
    mask: &mut dyn Mask,
    rng: &mut Rng,
    width: usize,
) -> Option<Vec<Line>> {
    let Some(name) = command.split_whitespace().next() else {
        return Some(vec![]);
    };
    Some(match name {
        "clear" | "cls" => return None,
        "ls" | "dir" => {
            let mut segs = vec![];
            for (i, (entry, style)) in LISTING.iter().enumerate() {
                if i > 0 {
                    segs.push(seg("  ", ""));
                }
                segs.push(seg(*entry, *style));
            }
            vec![Line {
                segments: segs,
                is_book: false,
                pos: None,
            }]
        }
        "pwd" => vec![plain_line(cwd().display().to_string(), "")],
        _ if command.trim() == "git status" => {
            GIT_STATUS.iter().map(|t| plain_line(*t, "")).collect()
        }
        _ if TOOLS.contains(&name) => (0..randint(rng, 4, 12))
            .map(|_| mask.noise(rng, width))
            .collect(),
        _ if cfg!(windows) => vec![plain_line(
            format!(
                "{name}: The term '{name}' is not recognized as a name of a cmdlet, function, script file, or executable program."
            ),
            "#f85149",
        )],
        _ => {
            let shell = std::env::var("SHELL").unwrap_or_default();
            let shell = Path::new(&shell)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or("bash".into());
            vec![plain_line(
                format!("{shell}: command not found: {name}"),
                "",
            )]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks::get_mask;
    use rand::SeedableRng;

    fn run(cmd: &str) -> Option<Vec<String>> {
        let mut m = get_mask("log", "python").unwrap();
        shell_output(cmd, m.as_mut(), &mut Rng::seed_from_u64(1), 80)
            .map(|ls| ls.iter().map(Line::plain).collect())
    }

    #[test]
    fn commands_get_believable_answers() {
        assert_eq!(run(""), Some(vec![]));
        assert_eq!(run("clear"), None);
        assert!(run("ls").unwrap()[0].contains("src"));
        assert_eq!(run("git status").unwrap()[0], "On branch main");
        let burst = run("npm test").unwrap();
        assert!((4..=12).contains(&burst.len()));
        let unknown = run("frobnicate --x").unwrap().join(" ");
        assert!(
            unknown.contains("frobnicate")
                && (unknown.contains("not found") || unknown.contains("not recognized"))
        );
    }

    #[test]
    fn prompt_ends_like_a_shell_prompt() {
        let p = shell_prompt();
        assert!(
            p.ends_with("$ ") || p.ends_with("% ") || p.ends_with("> "),
            "{p}"
        );
    }
}
