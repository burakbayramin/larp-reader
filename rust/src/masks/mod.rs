//! The disguises text is shown in. For now only their names; the masks
//! themselves are ported in stage 4.

/// In the order `m` cycles through them (`masks/__init__.py`).
pub const MASK_NAMES: &[&str] = &[
    "code", "log", "diff", "test", "build", "git", "docker", "agent",
];

/// Source languages of the code mask (`masks/code.py`).
pub const LANGS: &[&str] = &["python", "typescript", "go", "rust", "java", "csharp"];
