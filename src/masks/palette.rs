//! Editor colours (VS Code Dark+) and syntax highlighting for mask chrome (`masks/palette.py`).
//!
//! The Python version colours code with pygments, but only by token kind.
//! A small tokenizer per language gives the same colouring without a
//! grammar engine: comments, strings, numbers, keywords (control words in
//! their own colour), types, definitions and calls, constants and variables.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::base::Segment;

pub const BG: &str = "#1e1e1e";
pub const FG: &str = "#d4d4d4";
pub const GUTTER: &str = "#858585";
pub const COMMENT: &str = "#6a9955";
pub const KEYWORD: &str = "#569cd6";
pub const CONTROL: &str = "#c586c0";
pub const FUNC: &str = "#dcdcaa";
pub const TYPE: &str = "#4ec9b0";
pub const STRING: &str = "#ce9178";
pub const NUMBER: &str = "#b5cea8";
pub const VAR: &str = "#9cdcfe";
pub const CONST: &str = "#4fc1ff";

// terminal output
pub const GREEN: &str = "#3fb950";
pub const YELLOW: &str = "#d29922";
pub const RED: &str = "#f85149";
pub const BLUE: &str = "#58a6ff";
pub const CYAN: &str = "#39c5cf";
pub const DIM: &str = "#6e7681";

const CONTROL_WORDS: &[&str] = &[
    "if", "else", "elif", "for", "while", "return", "match", "import", "from", "use", "package",
    "try", "catch", "except", "finally", "await", "switch", "case", "break", "continue", "throw",
    "raise", "yield", "using", "defer", "foreach", "lock",
];

struct Lang {
    line_comment: &'static str,
    block_comment: bool,
    backtick_strings: bool,
    keywords: &'static [&'static str],
    /// Built-in type names (`Keyword.Type` in pygments).
    types: &'static [&'static str],
    /// Built-in functions: coloured as functions when lower-case (Python's builtins).
    builtins: &'static [&'static str],
    /// Words after which a name is being defined as a function.
    def_words: &'static [&'static str],
    /// Words after which a name is being defined as a type.
    type_words: &'static [&'static str],
}

const PYTHON: Lang = Lang {
    line_comment: "#",
    block_comment: false,
    backtick_strings: false,
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is",
        "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with",
        "yield", "match", "case", "True", "False", "None", "self", "cls",
    ],
    types: &[],
    builtins: &[
        "int",
        "str",
        "float",
        "bool",
        "bytes",
        "list",
        "dict",
        "tuple",
        "set",
        "frozenset",
        "object",
        "type",
        "len",
        "range",
        "print",
        "isinstance",
        "sorted",
        "min",
        "max",
        "sum",
        "any",
        "all",
        "open",
        "super",
        "enumerate",
        "zip",
        "map",
        "filter",
        "divmod",
        "iter",
        "next",
        "getattr",
        "setattr",
        "hasattr",
        "repr",
        "hash",
        "id",
        "abs",
        "round",
    ],
    def_words: &["def"],
    type_words: &["class"],
};

const TYPESCRIPT: Lang = Lang {
    line_comment: "//",
    block_comment: true,
    backtick_strings: true,
    keywords: &[
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "finally",
        "for",
        "from",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "let",
        "new",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "try",
        "typeof",
        "var",
        "while",
        "with",
        "yield",
        "async",
        "await",
        "of",
        "as",
        "implements",
        "interface",
        "private",
        "protected",
        "public",
        "static",
        "readonly",
        "type",
        "declare",
        "abstract",
        "true",
        "false",
        "null",
        "undefined",
    ],
    types: &[
        "string", "number", "boolean", "any", "void", "never", "unknown", "object", "symbol",
        "bigint",
    ],
    builtins: &[],
    def_words: &["function"],
    type_words: &["class", "interface", "type", "enum"],
};

const GO: Lang = Lang {
    line_comment: "//",
    block_comment: true,
    backtick_strings: true,
    keywords: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
        "true",
        "false",
        "nil",
        "iota",
    ],
    types: &[
        "bool",
        "byte",
        "complex64",
        "complex128",
        "error",
        "float32",
        "float64",
        "int",
        "int8",
        "int16",
        "int32",
        "int64",
        "rune",
        "string",
        "uint",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "uintptr",
        "any",
    ],
    builtins: &[
        "append", "cap", "close", "copy", "delete", "len", "make", "new", "panic", "print",
        "println", "recover",
    ],
    def_words: &["func"],
    type_words: &["type"],
};

const RUST: Lang = Lang {
    line_comment: "//",
    block_comment: true,
    backtick_strings: false,
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while",
    ],
    types: &[
        "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32",
        "i64", "i128", "isize", "f32", "f64",
    ],
    builtins: &[],
    def_words: &["fn"],
    type_words: &["struct", "enum", "trait", "type"],
};

const JAVA: Lang = Lang {
    line_comment: "//",
    block_comment: true,
    backtick_strings: false,
    keywords: &[
        "abstract",
        "assert",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "interface",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "volatile",
        "while",
        "var",
        "record",
        "yield",
        "true",
        "false",
        "null",
    ],
    types: &[
        "boolean", "byte", "char", "double", "float", "int", "long", "short", "void",
    ],
    builtins: &[],
    def_words: &[],
    type_words: &["class", "interface", "enum", "record"],
};

const CSHARP: Lang = Lang {
    line_comment: "//",
    block_comment: true,
    backtick_strings: false,
    keywords: &[
        "abstract",
        "as",
        "base",
        "break",
        "case",
        "catch",
        "checked",
        "class",
        "const",
        "continue",
        "default",
        "delegate",
        "do",
        "else",
        "enum",
        "event",
        "explicit",
        "extern",
        "finally",
        "fixed",
        "for",
        "foreach",
        "goto",
        "if",
        "implicit",
        "in",
        "interface",
        "internal",
        "is",
        "lock",
        "namespace",
        "new",
        "operator",
        "out",
        "override",
        "params",
        "private",
        "protected",
        "public",
        "readonly",
        "ref",
        "return",
        "sealed",
        "sizeof",
        "stackalloc",
        "static",
        "struct",
        "switch",
        "this",
        "throw",
        "try",
        "typeof",
        "unchecked",
        "unsafe",
        "using",
        "virtual",
        "volatile",
        "while",
        "async",
        "await",
        "var",
        "get",
        "set",
        "init",
        "record",
        "when",
        "yield",
        "nameof",
        "true",
        "false",
        "null",
    ],
    types: &[
        "bool", "byte", "char", "decimal", "double", "float", "int", "long", "object", "sbyte",
        "short", "string", "uint", "ulong", "ushort", "void", "dynamic",
    ],
    builtins: &[],
    def_words: &[],
    type_words: &["class", "interface", "struct", "enum", "record"],
};

fn lang(name: &str) -> &'static Lang {
    match name {
        "typescript" => &TYPESCRIPT,
        "go" => &GO,
        "rust" => &RUST,
        "java" => &JAVA,
        "csharp" => &CSHARP,
        _ => &PYTHON,
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// (text, style) tokens of `code`, which may span lines.
fn tokens(l: &Lang, code: &str) -> Vec<(String, &'static str)> {
    let s: Vec<char> = code.chars().collect();
    let mut out: Vec<(String, &'static str)> = vec![];
    let mut i = 0;
    let mut prev_word = String::new(); // the last identifier, for `def name` / `class Name`
    let text = |a: usize, b: usize| s[a..b].iter().collect::<String>();
    let starts = |i: usize, pat: &str| {
        pat.chars()
            .enumerate()
            .all(|(k, c)| s.get(i + k) == Some(&c))
    };
    while i < s.len() {
        let c = s[i];
        let start = i;
        // comments
        if starts(i, l.line_comment) {
            while i < s.len() && s[i] != '\n' {
                i += 1;
            }
            out.push((text(start, i), COMMENT));
            continue;
        }
        if l.block_comment && starts(i, "/*") {
            i += 2;
            while i < s.len() && !starts(i, "*/") {
                i += 1;
            }
            i = (i + 2).min(s.len());
            out.push((text(start, i), COMMENT));
            continue;
        }
        // strings, with Python prefixes (f"", r'', b"") and C# @"" / $""
        let prefix_len = {
            let mut k = 0;
            while i + k < s.len() && k < 2 && "fFrRbBuU@$".contains(s[i + k]) {
                k += 1;
            }
            if k > 0 && matches!(s.get(i + k), Some('"' | '\'')) {
                k
            } else {
                0
            }
        };
        let q = s[i + prefix_len];
        let quote_ok = q == '"' || q == '\'' || (q == '`' && l.backtick_strings);
        let rust_lifetime = std::ptr::eq(l, &RUST)
            && q == '\''
            && s.get(i + 2) != Some(&'\'')
            && s.get(i + 1) != Some(&'\\');
        if quote_ok && !rust_lifetime && (prefix_len > 0 || !is_ident(c) || c == q) {
            i += prefix_len;
            let triple = std::ptr::eq(l, &PYTHON) && starts(i, &q.to_string().repeat(3));
            let close: String = if triple {
                q.to_string().repeat(3)
            } else {
                q.to_string()
            };
            i += close.chars().count();
            let multiline = triple || q == '`';
            while i < s.len() {
                if s[i] == '\\' && q != '`' {
                    i += 2;
                    continue;
                }
                if starts(i, &close) {
                    i += close.chars().count();
                    break;
                }
                if s[i] == '\n' && !multiline {
                    break;
                }
                i += 1;
            }
            i = i.min(s.len());
            out.push((text(start, i), STRING));
            continue;
        }
        if c.is_ascii_digit() {
            while i < s.len()
                && (is_ident(s[i])
                    || (s[i] == '.' && s.get(i + 1).is_some_and(char::is_ascii_digit)))
            {
                i += 1;
            }
            out.push((text(start, i), NUMBER));
            continue;
        }
        if c == '@' && s.get(i + 1).copied().is_some_and(is_ident_start) {
            i += 1;
            while i < s.len() && (is_ident(s[i]) || s[i] == '.') {
                i += 1;
            }
            out.push((text(start, i), FUNC)); // decorators and annotations
            continue;
        }
        if is_ident_start(c) {
            while i < s.len() && is_ident(s[i]) {
                i += 1;
            }
            let word = text(start, i);
            let next = s.get(i).copied();
            let style = if l.types.contains(&word.as_str()) {
                TYPE
            } else if l.keywords.contains(&word.as_str()) {
                if CONTROL_WORDS.contains(&word.as_str()) {
                    CONTROL
                } else {
                    KEYWORD
                }
            } else if l.def_words.contains(&prev_word.as_str()) {
                FUNC
            } else if l.type_words.contains(&prev_word.as_str()) {
                TYPE
            } else if l.builtins.contains(&word.as_str()) || next == Some('(') || next == Some('!')
            {
                FUNC
            } else if word.starts_with(char::is_uppercase) {
                let upper = word.chars().all(|c| !c.is_lowercase());
                if upper && word.chars().count() > 1 {
                    CONST
                } else {
                    TYPE
                }
            } else {
                VAR
            };
            prev_word = word.clone();
            out.push((word, style));
            continue;
        }
        // whitespace, operators, punctuation
        i += 1;
        if !c.is_whitespace() {
            prev_word.clear();
        }
        out.push((c.to_string(), FG));
    }
    out
}

fn compute(lang_name: &str, code: &str) -> Vec<Vec<Segment>> {
    let mut lines: Vec<Vec<Segment>> = vec![vec![]];
    for (text, style) in tokens(lang(lang_name), code) {
        for (j, part) in text.split('\n').enumerate() {
            if j > 0 {
                lines.push(vec![]);
            }
            if part.is_empty() {
                continue;
            }
            let line = lines.last_mut().expect("at least one line");
            match line.last_mut() {
                Some((t, s)) if s == style => t.push_str(part),
                _ => line.push((part.to_string(), style.to_string())),
            }
        }
    }
    lines
}

/// (language, code) → highlighted lines.
type Cache = Mutex<HashMap<(String, String), Vec<Vec<Segment>>>>;

/// Syntax-coloured segments for each line of `code`.
pub fn highlight(lang_name: &str, code: &str) -> Vec<Vec<Segment>> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (lang_name.to_string(), code.to_string());
    if let Some(hit) = cache.lock().map(|c| c.get(&key).cloned()).ok().flatten() {
        return hit;
    }
    let lines = compute(lang_name, code);
    if let Ok(mut c) = cache.lock() {
        if c.len() > 20_000 {
            c.clear(); // filler is endless; keep memory bounded
        }
        c.insert(key, lines.clone());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style_of<'a>(line: &'a [Segment], text: &str) -> &'a str {
        &line
            .iter()
            .find(|(t, _)| t == text)
            .unwrap_or_else(|| panic!("{text} in {line:?}"))
            .1
    }

    #[test]
    fn highlight_colours_tokens_differently() {
        let lines = highlight("python", r#"def retry(limit: int = 5): return "x""#);
        let line = &lines[0];
        assert_eq!(style_of(line, "def"), KEYWORD);
        assert_eq!(style_of(line, "retry"), FUNC);
        assert_eq!(style_of(line, "5"), NUMBER);
        assert_eq!(style_of(line, "\"x\""), STRING);
        assert_eq!(style_of(line, "return"), CONTROL);
        assert_eq!(style_of(line, "limit"), VAR);
    }

    #[test]
    fn each_language_has_its_comments_strings_and_types() {
        let rust = &highlight("rust", "fn load(x: u32) -> Self { let s = \"a\"; } // done")[0];
        assert_eq!(style_of(rust, "load"), FUNC);
        assert_eq!(style_of(rust, "u32"), TYPE);
        assert_eq!(style_of(rust, "\"a\""), STRING);
        assert_eq!(style_of(rust, "// done"), COMMENT);
        let rust_lifetime = &highlight("rust", "fn f<'a>(x: &'a str) {}")[0];
        assert!(
            rust_lifetime.iter().all(|(_, s)| s != STRING),
            "{rust_lifetime:?}"
        );
        let go = &highlight("go", "func (c *Cache) Get(key string) error {")[0];
        assert_eq!(style_of(go, "Get"), FUNC);
        assert_eq!(style_of(go, "string"), TYPE);
        let ts = &highlight("typescript", "const MAX_SIZE = `t${x}`;")[0];
        assert_eq!(style_of(ts, "MAX_SIZE"), CONST);
        assert!(ts.iter().any(|(t, s)| t.starts_with('`') && s == STRING));
        let cs = &highlight("csharp", "[Obsolete] public sealed class Indexer {")[0];
        assert_eq!(style_of(cs, "Indexer"), TYPE);
        let py = &highlight("python", "@dataclass\nclass Span:")[..];
        assert_eq!(style_of(&py[0], "@dataclass"), FUNC);
        assert_eq!(style_of(&py[1], "Span"), TYPE);
    }

    #[test]
    fn multi_line_comments_and_docstrings_colour_every_line() {
        let lines = highlight("java", "/**\n * Indexes spans.\n */\nint x;");
        assert_eq!(lines.len(), 4);
        assert!(
            lines[..3]
                .iter()
                .all(|l| l.iter().all(|(_, s)| s == COMMENT))
        );
        let py = highlight("python", "x = \"\"\"a\nb\"\"\"\ny = 1");
        assert_eq!(py[1], vec![("b\"\"\"".to_string(), STRING.to_string())]);
    }
}
