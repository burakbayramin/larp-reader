//! A `psql` session: every paragraph is the one row a query returns, filler is
//! other statements and what the server answers, and a new chapter switches schema.

use std::sync::OnceLock;

use super::base::{Line, Segment, chrome, chrome_owned, progress_bar_label, seg};
use super::codegen::Deck;
use super::filler::{Slot, expand, seq, words};
use super::palette::{BLUE, CYAN, DIM, FG, GREEN, KEYWORD, NUMBER, STRING, YELLOW};
use super::{Mask, Rng, section};
use crate::book::{Paragraph, Pos};
use crate::text::{CODE_LINE_MAX, ragged, ragged_count};

const PROMPT: &str = "app=> ";
/// ` 1042 | ` before a row's first line, `      | ` before the rest.
const CELL: usize = 8;
/// Lines of a query and its result around the text lines.
const FRAME: usize = 6;
/// (table, column), chosen by line number.
const SOURCES: &[(&str, &str)] = &[
    ("notes", "body"),
    ("entries", "content"),
    ("drafts", "text"),
    ("messages", "body"),
    ("comments", "content"),
    ("annotations", "note"),
    ("snippets", "text"),
];

const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "UPDATE",
    "SET",
    "DELETE",
    "INSERT",
    "INTO",
    "VALUES",
    "AND",
    "ORDER",
    "BY",
    "LIMIT",
    "CREATE",
    "INDEX",
    "IF",
    "NOT",
    "EXISTS",
    "ON",
    "VACUUM",
    "ANALYZE",
    "BEGIN",
    "COMMIT",
    "EXPLAIN",
    "TO",
    "interval",
    "now()",
    "RETURNING",
    "TRUNCATE",
    "REINDEX",
    "TABLE",
];

/// A statement typed at the prompt, coloured: keywords, numbers, quoted strings.
fn statement(sql: &str, width: usize) -> Line {
    let mut segs = vec![seg(PROMPT, DIM)];
    for word in sql.split_inclusive(' ') {
        let bare = word.trim_end().trim_end_matches([';', ',', ')']);
        let look = if bare.starts_with('\'') || bare.ends_with('\'') {
            STRING
        } else if KEYWORDS.contains(&bare) {
            KEYWORD
        } else if !bare.is_empty() && bare.chars().all(|c| c.is_ascii_digit()) {
            NUMBER
        } else {
            FG
        };
        segs.push(seg(word, look));
    }
    chrome_owned(width, segs)
}

// -- filler ----------------------------------------------------------------------

fn vocab() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "tbl",
            words(&[
                "jobs", "users", "orders", "events", "sessions", "tokens", "audits", "invoices",
                "uploads", "webhooks", "tenants", "exports",
            ]),
        ),
        (
            "col",
            words(&[
                "status",
                "created_at",
                "updated_at",
                "owner_id",
                "tenant_id",
                "kind",
                "priority",
                "region",
            ]),
        ),
        (
            "st",
            words(&["'queued'", "'failed'", "'done'", "'stale'", "'pending'"]),
        ),
        ("id", seq(2_041, 29, 60)),
        ("n", seq(1, 3, 40)),
        ("d", seq(7, 7, 12)),
        ("ms", seq(0, 1, 40)),
        ("us", seq(117, 71, 12)),
    ]
}

/// A statement and the server's answer to it, a newline apart. The answer uses
/// only what the statement does, so no two entries share a statement.
const EXCHANGES: &[&str] = &[
    "UPDATE {tbl} SET {col} = now() WHERE id = {id};\nUPDATE 1",
    "UPDATE {tbl} SET status = {st} WHERE priority > {n} AND id < {id};\nUPDATE {n}",
    "DELETE FROM {tbl} WHERE created_at < now() - interval '{d} days';\nDELETE {d}",
    "VACUUM (ANALYZE) {tbl};\nVACUUM",
    "CREATE INDEX IF NOT EXISTS {tbl}_{col}_idx ON {tbl} ({col});\nNOTICE:  relation \"{tbl}_{col}_idx\" already exists, skipping",
    "INSERT INTO {tbl} (status, owner_id) VALUES ({st}, {id});\nINSERT 0 1",
    "REINDEX TABLE {tbl};\nREINDEX",
    "TRUNCATE {tbl}_staging;\nTRUNCATE TABLE",
];

fn exchanges_deck() -> &'static Deck {
    static D: OnceLock<Deck> = OnceLock::new();
    D.get_or_init(|| Deck::new(expand(EXCHANGES, &vocab(), 200), "sql-exchanges"))
}

fn answer(text: &str, width: usize) -> Line {
    let look = if text.starts_with("NOTICE") {
        YELLOW
    } else if text.starts_with("Time:") {
        DIM
    } else {
        FG
    };
    chrome(width, &[(text, look)])
}

/// The lines of exchange `i` of the pool: the statement, the answer, its timing.
fn exchange(i: usize, width: usize) -> Vec<Line> {
    let (sql, reply) = exchanges_deck()
        .get(i)
        .split_once('\n')
        .expect("exchanges have two lines");
    // writes take longer than the text's lookups, which stay under 10 ms
    let time = format!("Time: {}.{:03} ms", 10 + i % 89, i * 37 % 1000);
    vec![
        statement(sql, width),
        answer(reply, width),
        answer(&time, width),
    ]
}

#[derive(Default)]
pub struct SqlMask {
    slot: Slot,
    /// The exchange `noise` is reading, and which of its lines comes next.
    exchange: usize,
    part: usize,
}

impl Mask for SqlMask {
    fn name(&self) -> &'static str {
        "sql"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![
            chrome(width, &[("psql (16.4 (Debian 16.4-1.pgdg120+2))", FG)]),
            chrome(
                width,
                &[(
                    "SSL connection (protocol: TLSv1.3, cipher: TLS_AES_256_GCM_SHA384, compression: off)",
                    FG,
                )],
            ),
            chrome(width, &[("Type \"help\" for help.", FG)]),
            chrome(width, &[]),
            chrome(width, &[(PROMPT, DIM), ("\\timing on", CYAN)]),
            chrome(width, &[("Timing is on.", FG)]),
        ]
    }

    fn stream(&mut self) {
        self.slot.stream();
        self.part = 0;
    }

    /// Exchanges read line by line: statement, answer, timing.
    fn noise(&mut self, _rng: &mut Rng, width: usize) -> Line {
        if self.part == 0 {
            self.exchange = self.slot.take();
        }
        let line = exchange(self.exchange, width).swap_remove(self.part);
        self.part = (self.part + 1) % 3;
        line
    }

    /// Whole exchanges; the lines left over are empty prompts before them.
    fn noise_block(&mut self, count: usize, _rng: &mut Rng, width: usize) -> Option<Vec<Line>> {
        self.part = 0;
        let mut out = vec![chrome(width, &[(PROMPT, DIM)]); count % 3];
        for _ in 0..count / 3 {
            out.extend(exchange(self.slot.take(), width));
        }
        Some(out)
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged_count(&para.text, width.saturating_sub(CELL).max(1)) + FRAME
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        self.slot.set(line_no_start);
        self.part = 0;
        let (tbl, col) = SOURCES[line_no_start % SOURCES.len()];
        let id = 1_000 + line_no_start % 9_000;
        let rows = ragged(&para.text, width.saturating_sub(CELL).max(1));
        let rule = width.min(CODE_LINE_MAX).saturating_sub(7);
        let mut out = vec![
            statement(
                &format!("SELECT id, {col} FROM {tbl} WHERE id = {id};"),
                width,
            ),
            chrome(width, &[(&format!("  id  | {col}"), FG)]),
            chrome(width, &[(&format!("------+{}", "-".repeat(rule)), FG)]),
        ];
        for (i, t) in rows.into_iter().enumerate() {
            let cell = if i == 0 {
                format!(" {id} | ")
            } else {
                "      | ".to_string()
            };
            out.push(Line::book(vec![seg(cell, FG), seg(t, FG.book())], pos));
        }
        out.push(chrome(width, &[("(1 row)", FG)]));
        out.push(chrome(width, &[]));
        let ms = format!(
            "Time: {}.{:03} ms",
            line_no_start % 7,
            line_no_start * 37 % 1000
        );
        out.push(chrome(width, &[(&ms, DIM)]));
        out
    }

    /// Another schema put first on the search path.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let name = section::name(chapter, 0);
        vec![
            statement(&format!("SET search_path TO {name}, public;"), width),
            chrome(width, &[("SET", FG)]),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Fetching")
    }

    fn status_right(
        &self,
        _done: usize,
        _total: usize,
        _elapsed: std::time::Duration,
    ) -> Vec<Segment> {
        vec![
            seg("● app@db-primary:5432", GREEN),
            seg("  ·  ", DIM),
            seg("txn: idle", BLUE),
            seg("  ·  ", DIM),
            seg("UTF8", CYAN),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Sabah erkenden uyandı ve pencereyi açtı. ".repeat(4))
    }

    #[test]
    fn header_is_a_psql_banner() {
        let text: Vec<String> = SqlMask::default()
            .header(&mut Rng::seed_from_u64(1), 100)
            .iter()
            .map(Line::plain)
            .collect();
        assert!(text[0].starts_with("psql ("));
        assert!(text.contains(&"Type \"help\" for help.".to_string()));
    }

    #[test]
    fn a_paragraph_is_the_row_a_query_returns() {
        let lines = SqlMask::default().render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 9);
        let text: Vec<String> = lines.iter().map(Line::plain).collect();
        assert!(text[0].starts_with("app=> SELECT id, ") && text[0].ends_with(';'));
        assert!(text[1].starts_with("  id  | "));
        assert!(text[2].starts_with("------+--"));
        let book: Vec<String> = lines
            .iter()
            .filter(|l| l.is_book)
            .map(Line::plain)
            .collect();
        assert!(book.len() > 1);
        assert!(book[0].starts_with(" 1009 | "));
        assert!(book[1..].iter().all(|l| l.starts_with("      | ")));
        assert!(text.contains(&"(1 row)".to_string()));
        assert!(text.last().unwrap().starts_with("Time: "));
    }

    #[test]
    fn noise_is_statements_answers_and_timings_in_turn() {
        let mut m = SqlMask::default();
        let mut r = Rng::seed_from_u64(5);
        m.slot.set(0);
        for i in 0..42 {
            let p = m.noise(&mut r, 200).plain();
            assert_eq!(p.starts_with(PROMPT), i % 3 == 0, "{p}");
            assert_eq!(p.starts_with("Time: "), i % 3 == 2, "{p}");
        }
    }

    #[test]
    fn noise_blocks_are_whole_exchanges_of_the_exact_size() {
        for count in 0..=10 {
            let mut m = SqlMask::default();
            let block = m
                .noise_block(count, &mut Rng::seed_from_u64(1), 100)
                .unwrap();
            assert_eq!(block.len(), count);
            assert!(!block.iter().any(|l| l.is_book));
            if count >= 3 {
                assert!(block[count - 1].plain().starts_with("Time: "));
                assert!(block[count - 3].plain().starts_with(PROMPT));
            }
        }
    }

    #[test]
    fn statements_colour_keywords_and_numbers() {
        let l = statement("SELECT id FROM notes WHERE id = 1042;", 80);
        assert!(
            l.segments
                .iter()
                .any(|(t, s)| t == "SELECT " && *s == KEYWORD)
        );
        assert!(l.segments.iter().any(|(t, s)| t == "1042;" && *s == NUMBER));
    }

    #[test]
    fn pools_are_large_and_clean() {
        let d = exchanges_deck();
        assert!(d.len() >= 400, "{}", d.len());
        for i in 0..d.len() {
            let low = d.get(i).to_lowercase();
            assert!(!low.contains("book") && !low.contains("reader"), "{low}");
        }
    }
}
