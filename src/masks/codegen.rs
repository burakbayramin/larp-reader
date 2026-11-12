//! Large pools of generated filler, read in a shuffled order so nothing repeats soon (`masks/codegen.py`).
//!
//! Drawing filler from a short list repeats lines within a screen or two. A
//! `Deck` instead holds thousands of distinct items built from templates and
//! word lists, shuffled once with a fixed seed: reading it by a running index
//! gives every item once before any comes back, and the same index always
//! gives the same item, so a redrawn screen looks exactly as before.

use std::sync::OnceLock;

use rand::seq::SliceRandom;

use super::seeded;

/// Distinct items in a fixed shuffled order; `deck.get(i)` cycles through all of them.
pub struct Deck {
    items: Vec<String>,
}

impl Deck {
    pub fn new(items: impl IntoIterator<Item = String>, seed: &str) -> Deck {
        let mut seen = std::collections::HashSet::new();
        let mut unique: Vec<String> = items
            .into_iter()
            .filter(|i| seen.insert(i.clone()))
            .collect();
        unique.shuffle(&mut seeded(seed));
        Deck { items: unique }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, i: usize) -> &str {
        &self.items[i % self.items.len()]
    }
}

// -- vocabulary -----------------------------------------------------------------

const VARS: &[&str] = &[
    "chunk", "offset", "buf", "state", "count", "entry", "result", "payload", "node", "token",
    "span", "limit", "cursor", "batch", "record", "header", "digest", "delta", "window", "target",
    "source", "pending", "retries", "elapsed", "marker", "frame",
];
const ATTRS: &[&str] = &[
    "_store",
    "_cache",
    "_index",
    "_queue",
    "_lock",
    "_hits",
    "_misses",
    "_pending",
    "_handlers",
    "_config",
    "_stats",
    "_buffer",
    "_registry",
    "_spans",
    "_clock",
    "_backoff",
    "_seen",
    "_sink",
    "_pool",
    "_limits",
];
const FUNCS: &[&str] = &[
    "load",
    "flush",
    "evict",
    "resolve",
    "encode",
    "decode",
    "parse_header",
    "compact",
    "merge",
    "validate",
    "normalize",
    "dispatch",
    "rebuild",
    "snapshot",
    "refresh",
    "drain",
    "acquire",
    "release",
    "schedule",
    "collect",
    "split_chunks",
    "hydrate",
    "serialize",
    "checksum",
    "prune",
    "warm_up",
    "rotate",
    "reconcile",
];
const KEYS: &[&str] = &[
    "id",
    "name",
    "size",
    "offset",
    "kind",
    "path",
    "version",
    "status",
    "owner",
    "created_at",
    "ttl",
    "checksum",
    "encoding",
    "priority",
    "region",
    "tags",
];
const TYPES: &[&str] = &[
    "str", "int", "bytes", "float", "bool", "Path", "Span", "Node", "Entry", "Frame",
];
const NUMS: &[&str] = &[
    "0", "1", "2", "8", "16", "64", "128", "1024", "4096", "30.0", "0.5",
];

const STATEMENT_TEMPLATES: &[&str] = &[
    "{v} = self.{a}.get({k!r})",
    "{v} = self.{f}({w})",
    "{v} = {w}[{n}:]",
    "self.{a}[{k!r}] = {v}",
    "self.{a}.pop({v}, None)",
    "if {v} is None:",
    "if not self.{a}:",
    "if len({v}) > {n}:",
    "for {v} in self.{a}:",
    "for {v}, {w} in self.{a}.items():",
    "while {v} < {n}:",
    "return self.{f}({v})",
    "return {v}",
    "return {v}.{k}",
    "{v} += {n}",
    "self.{a} += {n}",
    "{v}: {t} = {w}",
    "{v} = {t}({w})",
    "{v} = {w} or {n}",
    "{v} = [{w} for {w} in self.{a} if {w}]",
    "{v} = {{{k!r}: {w}}}",
    "logger.debug({v!r}, {w})",
    "logger.info(\"{f} done\", extra={{{k!r}: {v}}})",
    "raise ValueError(f\"bad {k}: {{{v}!r}}\")",
    "assert {v} is not None",
    "with self.{a}:",
    "await self.{f}({v})",
    "{v} = await self.{f}({w})",
    "yield from self.{f}({v})",
    "del self.{a}[{v}]",
    "self.{a}.append({v})",
    "self.{a}.extend({v})",
    "{v} = sorted({w}, key=len)",
    "{v} = max({w}, {n})",
    "{v} = min({v}, {n})",
    "{v}, {w} = divmod({v}, {n})",
    "{v} = self.{a}.setdefault({k!r}, [])",
    "{v} = {w}.encode(\"utf-8\")",
    "{v} = {w}.strip()",
    "self.{f}({v}, {k}={w})",
];
const ARGS: &[&str] = &[
    "self",
    "self, key: str",
    "self, {v}: {t}",
    "self, {v}: {t}, {w}: int = {n}",
    "self, *, timeout: float = 30.0",
    "cls, {v}: {t}",
    "self, {v}: {t} | None = None",
];
const SIG_TEMPLATES: &[&str] = &[
    "def {f}({args}) -> None:",
    "def {f}({args}) -> {t}:",
    "async def {f}({args}) -> {t}:",
    "def _{f}({args}) -> bool:",
];

const DIRS: &[&str] = &[
    "src/core",
    "src/api",
    "src/db",
    "src/workers",
    "src/cli",
    "src/io",
    "src/auth",
    "src/sync",
];
const MODULES: &[&str] = &[
    "cache",
    "index",
    "queue",
    "scheduler",
    "handlers",
    "routes",
    "models",
    "session",
    "config",
    "storage",
    "parser",
    "encoder",
    "retry",
    "limits",
    "metrics",
    "events",
    "registry",
    "snapshot",
    "chunker",
    "compactor",
    "loader",
    "pipeline",
    "tokens",
    "backoff",
    "locks",
    "streams",
    "watcher",
    "migrate",
    "health",
    "spans",
];

const COMMAND_TEMPLATES: &[&str] = &[
    "pytest -q tests/test_{m}.py",
    "pytest -q tests/test_{m}.py -k {f}",
    "ruff check {d}/{m}.py",
    "mypy {d}/{m}.py",
    "python -m pytest tests/test_{m}.py::test_{f} -x",
    "rg \"def {f}\" {d}",
    "git diff --stat {d}",
    "git log --oneline -5 -- {d}/{m}.py",
];

const TASK_TEMPLATES: &[&str] = &[
    "Add tests for {m}.{f}",
    "Refactor {f} in {m}",
    "Handle empty input in {f}",
    "Update {m} docs",
    "Fix flaky test_{f}",
    "Type-check {m}",
];

/// Python's `str.format` for these templates: `{name}`, `{name!r}` (a quoted string) and `{{`/`}}`.
fn fill(template: &str, slots: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        let end = tail.find('}').expect("closed slot");
        let field = &tail[1..end];
        let (name, repr) = match field.strip_suffix("!r") {
            Some(n) => (n, true),
            None => (field, false),
        };
        let value = slots
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| *v)
            .expect("known slot");
        if repr {
            out.push('\'');
            out.push_str(value);
            out.push('\'');
        } else {
            out.push_str(value);
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn statements() -> Vec<String> {
    let mut out = vec![];
    let mut i = 0;
    for tpl in STATEMENT_TEMPLATES {
        for v in VARS {
            for w in VARS {
                if v != w {
                    // the remaining slots walk their own lists, so each (template, v, w) gets a fresh mix
                    out.push(fill(
                        tpl,
                        &[
                            ("v", v),
                            ("w", w),
                            ("a", ATTRS[i % ATTRS.len()]),
                            ("f", FUNCS[i % FUNCS.len()]),
                            ("k", KEYS[i % KEYS.len()]),
                            ("t", TYPES[i % TYPES.len()]),
                            ("n", NUMS[i % NUMS.len()]),
                        ],
                    ));
                }
                i += 1;
            }
        }
    }
    out
}

fn signatures() -> Vec<String> {
    let mut out = vec![];
    let mut i = 0;
    for tpl in SIG_TEMPLATES {
        for f in FUNCS {
            for args in ARGS {
                let filled = fill(
                    args,
                    &[
                        ("v", VARS[i % VARS.len()]),
                        ("w", VARS[(i + 7) % VARS.len()]),
                        ("t", TYPES[i % TYPES.len()]),
                        ("n", NUMS[i % NUMS.len()]),
                    ],
                );
                out.push(fill(
                    tpl,
                    &[
                        ("f", f),
                        ("args", &filled),
                        ("t", TYPES[(i + 3) % TYPES.len()]),
                    ],
                ));
                i += 1;
            }
        }
    }
    out
}

macro_rules! deck {
    ($name:ident, $build:expr, $seed:literal) => {
        pub fn $name() -> &'static Deck {
            static D: OnceLock<Deck> = OnceLock::new();
            D.get_or_init(|| Deck::new($build, $seed))
        }
    };
}

deck!(statements_deck, statements(), "statements");
deck!(signatures_deck, signatures(), "signatures");
deck!(
    files_deck,
    DIRS.iter()
        .flat_map(|d| MODULES.iter().map(move |m| format!("{d}/{m}.py"))),
    "files"
);
deck!(
    commands_deck,
    COMMAND_TEMPLATES
        .iter()
        .flat_map(|t| MODULES.iter().map(move |m| (t, m)))
        .enumerate()
        .map(|(i, (t, m))| fill(
            t,
            &[
                ("m", m),
                ("d", DIRS[i % DIRS.len()]),
                ("f", FUNCS[i % FUNCS.len()])
            ]
        )),
    "commands"
);
deck!(
    tasks_deck,
    TASK_TEMPLATES
        .iter()
        .flat_map(|t| MODULES.iter().flat_map(move |m| FUNCS[..6]
            .iter()
            .map(move |f| fill(t, &[("m", m), ("f", f)])))),
    "tasks"
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -- test_codegen.py --

    #[test]
    fn deck_never_repeats_within_its_size_and_is_stable() {
        let deck = Deck::new((0..50).map(|i| format!("item{i}")), "t");
        let first: Vec<&str> = (0..50).map(|i| deck.get(i)).collect();
        assert_eq!(first.iter().collect::<HashSet<_>>().len(), 50);
        assert_eq!((50..100).map(|i| deck.get(i)).collect::<Vec<_>>(), first);
        assert_eq!(
            Deck::new((0..50).map(|i| format!("item{i}")), "t").get(7),
            first[7]
        );
    }

    #[test]
    fn deck_order_is_shuffled_and_deduped() {
        let deck = Deck::new((0..50).map(|i| format!("item{i}")), "t");
        assert_ne!(
            (0..50).map(|i| deck.get(i).to_string()).collect::<Vec<_>>(),
            (0..50).map(|i| format!("item{i}")).collect::<Vec<_>>()
        );
        assert_eq!(Deck::new(["a", "b", "a"].map(String::from), "x").len(), 2);
    }

    #[test]
    fn pools_are_large() {
        assert!(
            statements_deck().len() >= 2000,
            "{}",
            statements_deck().len()
        );
        assert!(signatures_deck().len() >= 300);
        assert!(files_deck().len() >= 200);
        assert!(commands_deck().len() >= 200);
        assert!(tasks_deck().len() >= 100);
    }

    #[test]
    fn templates_fill_like_python_format() {
        assert_eq!(
            fill(
                "{v} = {{{k!r}: {w}}}",
                &[("v", "a"), ("k", "id"), ("w", "b")]
            ),
            "a = {'id': b}"
        );
        assert_eq!(
            fill(
                r#"raise ValueError(f"bad {k}: {{{v}!r}}")"#,
                &[("k", "id"), ("v", "x")]
            ),
            r#"raise ValueError(f"bad id: {x!r}")"#
        );
    }

    #[test]
    fn no_forbidden_words_in_pools() {
        for deck in [
            statements_deck(),
            signatures_deck(),
            files_deck(),
            commands_deck(),
            tasks_deck(),
        ] {
            for i in 0..deck.len() {
                let low = deck.get(i).to_lowercase();
                assert!(
                    !["book", "reader", "okuyucu"]
                        .iter()
                        .any(|w| low.contains(w)),
                    "{low}"
                );
            }
        }
    }
}
