//! Filler pools for the log, diff, test, build, docker and git masks, and the
//! helpers the other masks build their own pools with.
//!
//! Like `codegen`, each pool is a `Deck`: hundreds to thousands of distinct lines
//! built from templates and word lists, shuffled once, read by a running slot
//! so a line comes back only after the whole pool has been shown.

use std::sync::OnceLock;

use super::codegen::{DIRS, Deck, FUNCS, MODULES, fill};

/// Where the hidden screen's endless stream starts before any text is drawn:
/// far from any paragraph's line numbers.
pub const STREAM_BASE: usize = 1_000_003;

/// How far ahead of the text the hidden stream reads: half the smallest pool,
/// as far as a pool gets from the text's place both ways (pools wrap around).
pub const STREAM_GAP: usize = 200;

/// The index a mask reads its pools at. `render` sets it from the paragraph's
/// line number; every filler line then takes the next one.
///
/// The hidden screen's stream must not read on from there: that is the filler
/// around the text just read. `stream` moves it `STREAM_GAP` ahead of the
/// text's place, or back to where the last stream stopped if the text has not
/// moved since (a redraw, hiding again), so a hide repeats neither.
pub struct Slot {
    at: usize,
    /// While the stream reads: the text's place it started from.
    stream_from: Option<usize>,
    /// The last stream a render cut off: the text's place it started from, and where it stopped.
    paused: Option<(usize, usize)>,
}

impl Default for Slot {
    fn default() -> Self {
        Slot {
            at: STREAM_BASE,
            stream_from: Some(STREAM_BASE),
            paused: None,
        }
    }
}

impl Slot {
    pub fn set(&mut self, n: usize) {
        if let Some(from) = self.stream_from.take() {
            self.paused = Some((from, self.at));
        }
        self.at = n;
    }

    pub fn take(&mut self) -> usize {
        self.at += 1;
        self.at - 1
    }

    /// Read the hidden stream from here on, away from the text's filler.
    pub fn stream(&mut self) {
        if self.stream_from.is_some() {
            return; // already streaming
        }
        let text = self.at;
        self.at = match self.paused {
            Some((from, stopped)) if from == text => stopped,
            _ => text + STREAM_GAP,
        };
        self.stream_from = Some(text);
    }
}

/// `count` numbers from `start`, `step` apart, as text.
pub(super) fn seq(start: usize, step: usize, count: usize) -> Vec<String> {
    (0..count).map(|i| (start + i * step).to_string()).collect()
}

pub(super) fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Each template filled from the cross product of the word lists it mentions,
/// sampled in a scattered order and cut to `cap` lines.
pub(super) fn expand(templates: &[&str], vocab: &[(&str, Vec<String>)], cap: usize) -> Vec<String> {
    let mut out = vec![];
    for tpl in templates {
        let used: Vec<&(&str, Vec<String>)> = vocab
            .iter()
            .filter(|(name, _)| tpl.contains(&format!("{{{name}}}")))
            .collect();
        let total = used
            .iter()
            .fold(1usize, |t, (_, v)| t.saturating_mul(v.len()));
        for i in 0..total.min(cap) {
            let mut at = i.wrapping_mul(7919) % total;
            let slots: Vec<(&str, &str)> = used
                .iter()
                .map(|(name, values)| {
                    let v = &values[at % values.len()];
                    at /= values.len();
                    (*name, v.as_str())
                })
                .collect();
            out.push(fill(tpl, &slots));
        }
    }
    out
}

// -- log -------------------------------------------------------------------------

fn log_vocab() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "res",
            words(&[
                "users", "orders", "jobs", "sessions", "invoices", "tokens", "items", "events",
                "reports", "uploads", "carts", "metrics", "webhooks", "accounts", "payments",
                "images", "queues", "tenants", "devices", "exports", "alerts", "audits", "files",
                "groups",
            ]),
        ),
        (
            "host",
            words(&[
                "node-a1", "node-a2", "node-b1", "node-b2", "node-c1", "edge-1", "edge-2",
                "edge-3", "store-1", "store-2", "mq-1", "mq-2",
            ]),
        ),
        ("ms", seq(3, 17, 40)),
        ("n", seq(16, 37, 50)),
        ("id", seq(4821, 13, 60)),
        ("p", seq(41, 2, 30)),
        ("k", seq(0, 1, 16)),
        ("seg", seq(7, 3, 40)),
        ("kb", seq(64, 96, 30)),
        ("sz", seq(4, 2, 12)),
        ("idle", seq(1, 1, 8)),
        ("ttl", seq(60, 60, 20)),
        ("st", words(&["200", "201", "204", "304"])),
        (
            "sid",
            (0..30)
                .map(|i| format!("{:06x}", 0x9f3a1c + i * 40_503))
                .collect(),
        ),
        ("rev", seq(118, 7, 30)),
    ]
}

const LOG_INFO: &[&str] = &[
    "request completed in {ms}ms path=/v1/{res}",
    "request completed in {ms}ms status={st} path=/v1/{res}/{id}",
    "connection pool size={sz} idle={idle}",
    "flushed {n} records to disk",
    "flushed {n} records to {res} segment {seg}",
    "heartbeat ok seq={n}",
    "heartbeat ok peer={host} rtt={ms}ms",
    "job {id} finished status=done",
    "job {id} finished in {ms}ms queue={res}",
    "cache hit ratio={p}% shard={k}",
    "compacted segment {seg} reclaimed={kb}KB",
    "session {sid} refreshed user={id}",
    "token issued sub=user:{id} ttl={ttl}s",
    "consumer lag topic={res} partition={k} lag={n}",
    "snapshot {seg} written in {ms}ms",
    "migrated {n} rows table={res}",
    "rate limit window reset key=user:{id}",
    "health check {host} ok latency={ms}ms",
    "batch {id} committed offset={n}",
    "metrics exported series={n} took={ms}ms",
    "config reloaded rev={rev}",
    "gc pause {ms}ms freed={kb}KB",
    "webhook delivered id={id} attempt=1 status={st}",
    "index rebuilt for {res} in {ms}ms",
    "lease renewed holder={host} ttl={ttl}s",
];
const LOG_DEBUG: &[&str] = &[
    "cache lookup key=user:{id} hit=true",
    "cache lookup key={res}:{id} hit=false",
    "retry budget remaining={k}",
    "parsed payload bytes={n}",
    "scheduler tick queue_depth={k} elapsed={ms}ms",
    "serialized {res} id={id} bytes={n}",
    "connection {sid} acquired from pool wait={ms}ms",
    "shard {k} watermark={n}",
    "span {sid} closed duration={ms}ms",
    "dns lookup {host} resolved in {ms}ms",
    "lock {res}:{id} acquired holder={host}",
    "evicted {k} entries from {res} cache",
];
const LOG_WARN: &[&str] = &[
    "slow query detected took={ms}ms table={res}",
    "retrying upstream call host={host} attempt={k}",
    "queue depth above soft limit depth={n} queue={res}",
    "cache miss storm shard={k} ratio={p}%",
    "clock skew {ms}ms against {host}",
    "connection {sid} reset by peer {host}",
    "slow consumer topic={res} lag={n}",
    "disk usage at {p}% on {host}",
    "request exceeded budget {ms}ms path=/v1/{res}",
];

// -- docker ----------------------------------------------------------------------

fn docker_vocab() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "res",
            words(&[
                "jobs", "users", "orders", "reports", "uploads", "tokens", "events", "items",
                "sessions", "exports", "invoices", "webhooks",
            ]),
        ),
        ("port", seq(41200, 14, 40)),
        ("ip", seq(2, 1, 12)),
        ("id", seq(4821, 11, 60)),
        ("ms", seq(7, 19, 40)),
        ("n", seq(12, 9, 50)),
        ("p", seq(1, 1, 12)),
        ("s", seq(0, 1, 30)),
        ("mm", seq(0, 1, 40)),
        ("ss", seq(0, 1, 60)),
        ("ms3", seq(100, 37, 24)),
        ("mb", seq(3, 2, 20)),
        ("k", seq(1, 1, 5)),
        ("bytes", seq(240, 331, 40)),
        (
            "st",
            words(&[
                "200 OK",
                "201 Created",
                "202 Accepted",
                "204 No Content",
                "404 Not Found",
                "422 Unprocessable Entity",
            ]),
        ),
        ("method", words(&["GET", "POST", "PUT", "GET", "GET"])),
        (
            "tbl",
            words(&[
                "jobs", "users", "orders", "events", "sessions", "tokens", "audits", "reports",
            ]),
        ),
        (
            "asset",
            words(&[
                "app.js",
                "app.css",
                "vendor.js",
                "logo.svg",
                "favicon.ico",
                "chunk-12.js",
                "chunk-34.js",
                "fonts/inter.woff2",
                "img/hero.webp",
                "runtime.js",
            ]),
        ),
        (
            "task",
            words(&[
                "cleanup",
                "reindex",
                "send_digest",
                "expire_sessions",
                "rotate_keys",
                "sync_billing",
                "prune_exports",
                "refresh_cache",
            ]),
        ),
    ]
}

const API_TPL: &[&str] = &[
    "INFO:     172.18.0.{ip}:{port} - \"{method} /v1/{res} HTTP/1.1\" {st}",
    "INFO:     172.18.0.{ip}:{port} - \"GET /v1/{res}/{id} HTTP/1.1\" {st}",
    "INFO:     172.18.0.{ip}:{port} - \"POST /v1/{res} HTTP/1.1\" {st}",
    "INFO:     request id={id} path=/v1/{res} took {ms} ms",
    "WARNING:  slow response from upstream ({ms} ms)",
    "WARNING:  retrying {res} fetch attempt {k}/5",
    "INFO:     worker pid {n} handled {id} requests",
    "DEBUG:    cache hit for /v1/{res}/{id}",
];
const DB_TPL: &[&str] = &[
    "LOG:  checkpoint complete: wrote {n} buffers ({p}.{k}%)",
    "LOG:  checkpoint starting: time",
    "LOG:  autovacuum: processing table \"public.{tbl}\"",
    "LOG:  automatic vacuum of table \"public.{tbl}\": {n} pages, {id} tuples removed",
    "LOG:  duration: {ms}.{k}00 ms  statement: SELECT * FROM {tbl} WHERE id = {id}",
    "LOG:  duration: {ms} ms  execute <unnamed>: UPDATE {tbl} SET seen = now() WHERE id = {id}",
    "LOG:  connection received: host=172.18.0.{ip} port={port}",
    "LOG:  disconnection: session time: 0:0{k}:{ss} user=app database=app",
];
const WEB_TPL: &[&str] = &[
    "172.18.0.1 - - [11/Nov/2026:09:{mm}:{ss} +0000] \"GET /static/{asset} HTTP/1.1\" 304 0",
    "172.18.0.1 - - [11/Nov/2026:09:{mm}:{ss} +0000] \"GET /{res} HTTP/1.1\" 200 {bytes}",
    "172.18.0.{ip} - - [11/Nov/2026:09:{mm}:{ss} +0000] \"{method} /api/{res}/{id} HTTP/1.1\" 200 {bytes}",
    "2026/11/11 09:{mm}:{ss} [notice] 1#1: worker process {n} exited with code 0",
    "2026/11/11 09:{mm}:{ss} [info] 1#1: *{id} client closed connection while waiting for request",
];
const REDIS_TPL: &[&str] = &[
    "1:M 11 Nov 2026 09:{mm}:{ss}.{ms3} * Background saving started by pid {n}",
    "1:M 11 Nov 2026 09:{mm}:{ss}.{ms3} * DB saved on disk",
    "1:M 11 Nov 2026 09:{mm}:{ss}.{ms3} * {n} changes in {s} seconds. Saving...",
    "1:M 11 Nov 2026 09:{mm}:{ss}.{ms3} * Fork CoW for RDB: current {mb} MB, peak {n} MB",
    "1:M 11 Nov 2026 09:{mm}:{ss}.{ms3} * Background saving terminated with success",
    "1:M 11 Nov 2026 09:{mm}:{ss}.{ms3} - Accepted 172.18.0.{ip}:{port}",
];
const WORKER_TPL: &[&str] = &[
    "[tasks] job {id} received",
    "[tasks] job {id} succeeded in 0.{ms}s",
    "[tasks] job {id} queue={res} picked up by pid {n}",
    "[beat] scheduler: sending due task {task}",
    "[beat] scheduler: {task} next run in {n}s",
    "[tasks] retry job {id} attempt {k}/5",
    "[tasks] batch {id} flushed {n} results",
];

// -- git -------------------------------------------------------------------------

fn git_paths() -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for (i, m) in MODULES.iter().enumerate() {
        for d in DIRS {
            out.push(format!("{d}/{m}.py"));
        }
        out.push(format!("tests/test_{m}.py"));
        out.push(format!("docs/{m}.md"));
        out.push(format!("config/{m}.yaml"));
        out.push(format!("scripts/{m}.sh"));
        out.push(format!(".github/workflows/{m}.yml"));
        out.push(format!("src/db/migrations/{:04}_{m}.py", 30 + i));
    }
    out
}

// -- pytest ids ------------------------------------------------------------------

fn test_ids() -> Vec<String> {
    let cases = words(&[
        "returns_none",
        "handles_empty",
        "rejects_bad_input",
        "roundtrip",
        "is_idempotent",
        "raises_on_timeout",
        "keeps_order",
        "skips_duplicates",
        "retries_once",
        "preserves_unicode",
        "wraps_lines",
        "caches_result",
        "parses_chunk",
        "respects_limit",
        "logs_warning",
        "closes_handle",
        "merges_defaults",
        "validates_schema",
        "truncates_long",
        "emits_event",
    ]);
    expand(
        &["tests/test_{m}.py::test_{f}_{c}"],
        &[("m", words(MODULES)), ("f", words(FUNCS)), ("c", cases)],
        3000,
    )
}

// -- build -----------------------------------------------------------------------

const CRATE_NAMES: &[&str] = &[
    "serde",
    "serde_json",
    "serde_derive",
    "tokio",
    "tokio_util",
    "regex",
    "regex_syntax",
    "anyhow",
    "thiserror",
    "hyper",
    "hyper_util",
    "clap",
    "clap_builder",
    "memchr",
    "once_cell",
    "bytes",
    "itoa",
    "ryu",
    "libc",
    "cfg_if",
    "log",
    "env_logger",
    "tracing",
    "tracing_core",
    "futures",
    "futures_util",
    "futures_core",
    "pin_project",
    "pin_utils",
    "http",
    "http_body",
    "httparse",
    "mio",
    "socket_pair",
    "parking_lot",
    "parking_lot_core",
    "smallvec",
    "lazy_static",
    "indexmap",
    "hashbrown",
    "rand",
    "rand_core",
    "rand_chacha",
    "ppv_lite",
    "getrandom",
    "unicode_ident",
    "unicode_width",
    "proc_macro",
    "quote",
    "syn",
    "autocfg",
    "num_traits",
    "num_cpus",
    "chrono",
    "time",
    "uuid",
    "url",
    "percent_encoding",
    "idna",
    "base_codec",
    "bitflags",
    "byteorder",
    "crossbeam_utils",
    "crossbeam_channel",
    "either",
    "fnv",
    "tempfile",
    "walkdir",
    "same_file",
    "glob",
    "semver",
    "toml",
    "ahash",
    "sha_core",
    "digest",
    "crypto_common",
    "generic_array",
    "zeroize",
    "rustls",
    "webpki",
    "ring",
    "tower",
    "tower_service",
    "hpack",
    "tokio_rustls",
    "reqwest",
];

fn crate_lines() -> Vec<String> {
    let mut out = vec![];
    for (ci, name) in CRATE_NAMES.iter().enumerate() {
        for k in 0..12 {
            out.push(format!(
                "{name} v{}.{}.{}",
                (ci * 7 + k * 3) % 4,
                (ci * 5 + k * 11) % 30,
                (ci + k * 13) % 15
            ));
        }
    }
    out
}

// -- decks -----------------------------------------------------------------------

macro_rules! deck {
    ($name:ident, $build:expr, $seed:literal) => {
        pub fn $name() -> &'static Deck {
            static D: OnceLock<Deck> = OnceLock::new();
            D.get_or_init(|| Deck::new($build, $seed))
        }
    };
}

deck!(
    log_info_deck,
    expand(LOG_INFO, &log_vocab(), 260),
    "log-info"
);
deck!(
    log_debug_deck,
    expand(LOG_DEBUG, &log_vocab(), 260),
    "log-debug"
);
deck!(
    log_warn_deck,
    expand(LOG_WARN, &log_vocab(), 260),
    "log-warn"
);
deck!(
    docker_api_deck,
    expand(API_TPL, &docker_vocab(), 120),
    "docker-api"
);
deck!(
    docker_db_deck,
    expand(DB_TPL, &docker_vocab(), 120),
    "docker-db"
);
deck!(
    docker_web_deck,
    expand(WEB_TPL, &docker_vocab(), 150),
    "docker-web"
);
deck!(
    docker_redis_deck,
    expand(REDIS_TPL, &docker_vocab(), 150),
    "docker-redis"
);
deck!(
    docker_worker_deck,
    expand(WORKER_TPL, &docker_vocab(), 120),
    "docker-worker"
);
deck!(git_paths_deck, git_paths(), "git-paths");
deck!(test_ids_deck, test_ids(), "test-ids");
deck!(crates_deck, crate_lines(), "crates");

/// The log pool of a level.
pub fn log_deck(level: &str) -> &'static Deck {
    match level {
        "DEBUG" => log_debug_deck(),
        "WARN" => log_warn_deck(),
        _ => log_info_deck(),
    }
}

/// The pool of a compose service's log.
pub fn docker_deck(svc: &str) -> &'static Deck {
    match svc {
        "api-1" => docker_api_deck(),
        "db-1" => docker_db_deck(),
        "web-1" => docker_web_deck(),
        "redis-1" => docker_redis_deck(),
        _ => docker_worker_deck(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<(&'static str, &'static Deck)> {
        vec![
            ("log info", log_info_deck()),
            ("log debug", log_debug_deck()),
            ("log warn", log_warn_deck()),
            ("docker api", docker_api_deck()),
            ("docker db", docker_db_deck()),
            ("docker web", docker_web_deck()),
            ("docker redis", docker_redis_deck()),
            ("docker worker", docker_worker_deck()),
            ("git paths", git_paths_deck()),
            ("test ids", test_ids_deck()),
            ("crates", crates_deck()),
        ]
    }

    #[test]
    fn pools_are_large() {
        for (name, deck) in all() {
            assert!(deck.len() >= 400, "{name}: {}", deck.len());
        }
        assert!(log_info_deck().len() >= 2000);
        assert!(test_ids_deck().len() >= 2000);
    }

    #[test]
    fn no_forbidden_words_in_pools() {
        for (name, deck) in all() {
            for i in 0..deck.len() {
                let low = deck.get(i).to_lowercase();
                assert!(
                    !["book", "reader", "okuyucu"]
                        .iter()
                        .any(|w| low.contains(w)),
                    "{name}: {low}"
                );
            }
        }
    }

    #[test]
    fn slot_starts_far_from_line_numbers_and_counts_up() {
        let mut s = Slot::default();
        assert_eq!(s.take(), STREAM_BASE);
        s.set(40);
        assert_eq!((s.take(), s.take()), (40, 41));
    }

    #[test]
    fn the_stream_reads_away_from_the_text_and_goes_on_where_it_stopped() {
        let mut s = Slot::default();
        s.set(40);
        s.take();
        s.stream();
        assert_eq!((s.take(), s.take()), (41 + STREAM_GAP, 42 + STREAM_GAP));
        s.stream(); // still streaming: no jump
        assert_eq!(s.take(), 43 + STREAM_GAP);
        s.set(40); // the same screen drawn again
        s.take();
        s.stream();
        assert_eq!(s.take(), 44 + STREAM_GAP);
        s.set(90); // read on
        s.stream();
        assert_eq!(s.take(), 90 + STREAM_GAP);
    }

    #[test]
    fn crate_lines_are_names_with_semver() {
        for i in 0..crates_deck().len() {
            let (name, ver) = crates_deck().get(i).split_once(" v").unwrap();
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{name}"
            );
            assert_eq!(
                ver.split('.').filter(|x| x.parse::<u32>().is_ok()).count(),
                3
            );
        }
    }

    // -- no repeats, through the masks --

    use crate::book::{Book, Chapter, Paragraph};
    use crate::masks::{get_mask, seeded};
    use crate::session::Session;
    use std::collections::HashMap;

    /// The filler content of a line, if it is a filler line of the mask (counts and times left out).
    fn key(mask: &str, line: &crate::masks::Line) -> Option<String> {
        if line.is_book {
            return None;
        }
        let p = line.plain();
        match mask {
            "log" => line.segments.last().map(|(t, _)| t.clone()),
            "diff" => (p.starts_with("  ") || p.starts_with("- ")).then(|| p.clone()),
            "test" => p
                .contains(" PASSED")
                .then(|| p.split(" PASSED").next().unwrap().to_string()),
            "build" => p.contains("Compiling").then(|| p.clone()),
            "docker" => p.contains(" | ").then(|| p.clone()),
            "kubectl" => p.starts_with('[').then(|| p.clone()),
            "terraform" => (p.contains(": Refreshing") || p.contains(": Read")).then(|| p.clone()),
            // statements other than the text's query, and their timings (the query's is under 10 ms)
            "sql" => ((p.len() > 6 && p.starts_with("app=> ") && !p.contains("SELECT id, "))
                || (p.starts_with("Time: ") && p.len() > 14))
                .then(|| p.clone()),
            "jupyter" => p
                .split_once("]: ")
                .map(|(_, c)| c.to_string())
                .filter(|c| !c.contains(".loc[")),
            _ => p
                .contains(" | ")
                .then(|| p.split(" | ").next().unwrap().to_string()),
        }
    }

    const MASKS: [&str; 10] = [
        "log",
        "diff",
        "test",
        "build",
        "docker",
        "git",
        "kubectl",
        "terraform",
        "sql",
        "jupyter",
    ];

    fn assert_spread(mask: &str, keys: Vec<Option<String>>, window: usize) {
        let mut last: HashMap<String, usize> = HashMap::new();
        let mut seen = 0;
        for (i, k) in keys.into_iter().enumerate() {
            let Some(k) = k else { continue };
            seen += 1;
            if let Some(prev) = last.insert(k.clone(), i) {
                assert!(i - prev > window, "{mask}: {k:?} again after {}", i - prev);
            }
        }
        assert!(seen > 100, "{mask}: only {seen} filler lines");
    }

    #[test]
    fn hidden_stream_does_not_repeat_within_300_lines() {
        for name in MASKS {
            let mut m = get_mask(name, "python").unwrap();
            let mut rng = seeded("stream");
            let keys = (0..3000)
                .map(|_| key(name, &m.noise(&mut rng, 120)))
                .collect();
            assert_spread(name, keys, 300);
        }
    }

    #[test]
    fn consecutive_paragraphs_do_not_repeat_filler_within_a_few_screens() {
        let text = "Oda dar ve basıktı, tavanı alçaktı; pencereden giren ışık azdı. ";
        for name in MASKS {
            let book = Book {
                id: "bk".into(),
                chapters: vec![Chapter {
                    title: Some("1".into()),
                    paragraphs: (0..400)
                        .map(|i| Paragraph::new(format!("{}{i}", text.repeat(2 + i % 3))))
                        .collect(),
                }],
            };
            let mut s = Session::new(book, get_mask(name, "python").unwrap(), 5, 120, (0, 0));
            let lines = s.lines_from((0, 0), 1500);
            let keys = lines.iter().map(|l| key(name, l)).collect();
            assert_spread(name, keys, 150);
        }
    }

    #[test]
    fn the_hidden_stream_reads_where_the_text_never_does() {
        let text = "Oda dar ve basıktı, tavanı alçaktı; pencereden giren ışık azdı. ";
        let book = Book {
            id: "bk".into(),
            chapters: vec![Chapter {
                title: Some("1".into()),
                paragraphs: (0..60)
                    .map(|i| Paragraph::new(format!("{}{i}", text.repeat(2 + i % 3))))
                    .collect(),
            }],
        };
        for name in MASKS {
            let mut s = Session::new(
                book.clone(),
                get_mask(name, "python").unwrap(),
                5,
                120,
                (0, 0),
            );
            let mut rng = seeded("hidden");
            for p in [3, 40] {
                // the filler of the screens around paragraph `p`
                let near: std::collections::HashSet<String> = (p - 3..p + 6)
                    .flat_map(|q| s.paragraph_lines((0, q)))
                    .filter_map(|l| key(name, &l))
                    .collect();
                let mut hidden = vec![];
                for _ in 0..2 {
                    // the screen drawn, then hidden: twice at the same place
                    let _ = s.paragraph_lines((0, p));
                    s.mask_mut().stream();
                    hidden.extend(
                        (0..60).filter_map(|_| key(name, &s.mask_mut().noise(&mut rng, 120))),
                    );
                }
                assert!(hidden.len() > 40, "{name}");
                for k in &hidden {
                    assert!(!near.contains(k), "{name}: {k:?} is filler around the text");
                }
                let unique: std::collections::HashSet<&String> = hidden.iter().collect();
                assert_eq!(
                    unique.len(),
                    hidden.len(),
                    "{name}: hiding again repeats the last hide"
                );
            }
        }
    }

    #[test]
    fn redrawing_a_paragraph_gives_identical_lines() {
        let book = Book {
            id: "bk".into(),
            chapters: vec![Chapter {
                title: Some("1".into()),
                paragraphs: (0..30)
                    .map(|i| Paragraph::new(format!("Oda dar ve basıktı {i}. ").repeat(5)))
                    .collect(),
            }],
        };
        for name in MASKS {
            let mut s = Session::new(
                book.clone(),
                get_mask(name, "python").unwrap(),
                5,
                100,
                (0, 0),
            );
            let first: Vec<String> = s
                .paragraph_lines((0, 20))
                .iter()
                .map(|l| l.plain())
                .collect();
            let _ = s.paragraph_lines((0, 3));
            let again: Vec<String> = s
                .paragraph_lines((0, 20))
                .iter()
                .map(|l| l.plain())
                .collect();
            assert_eq!(first, again, "{name}");
        }
    }
}
