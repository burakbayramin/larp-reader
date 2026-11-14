//! A service log (`masks/log.py`): text lines are INFO entries of one worker, filler is other services' chatter.

use super::base::{Line, Segment, chrome, chrome_owned, progress_bar_label, seg};
use super::palette::{BLUE, CYAN, DIM, FG, GREEN, YELLOW};
use super::time::{EPOCH_DAY, civil};
use super::{Mask, Rng, pick, randint, section, weighted};
use crate::book::{Paragraph, Pos};
use crate::text::ragged;
use unicode_width::UnicodeWidthStr;

/// Milliseconds of 2026-11-11 09:00:00 after that day's midnight: the log's first moment.
const EPOCH_MS: u64 = 9 * 3_600_000;
const BOOK_SVC: &str = "svc.worker";
const SVC_WIDTH: usize = 10;
const SERVICES: &[&str] = &[
    "svc.worker",
    "svc.api",
    "svc.cache",
    "svc.queue",
    "svc.auth",
];
const LEVELS: &[&str] = &["INFO", "DEBUG", "WARN"];
/// Below this the ISO prefix leaves too little room for text.
const FULL_MIN_WIDTH: usize = 100;
const COMPACT_MIN_WIDTH: usize = 60;
/// Below the per-line budget of render(), so time never runs backwards.
const MAX_STEP_MS: u64 = 700;
const LINE_MS: u64 = 731;

fn messages(level: &str) -> &'static [&'static str] {
    match level {
        "DEBUG" => &[
            "cache lookup key=user:1042 hit=true",
            "retry budget remaining=3",
            "parsed payload bytes=2048",
            "scheduler tick queue_depth=0",
        ],
        "WARN" => &[
            "slow query detected took=840ms",
            "retrying upstream call attempt=2",
            "queue depth above soft limit",
        ],
        _ => &[
            "request completed in 12ms",
            "connection pool size=8 idle=5",
            "flushed 128 records to disk",
            "heartbeat ok",
            "job 4821 finished status=done",
        ],
    }
}

fn level_style(level: &str) -> String {
    match level {
        "DEBUG" => BLUE.to_string(),
        "WARN" => format!("bold {YELLOW}"),
        _ => GREEN.to_string(),
    }
}

fn short_level(level: &str) -> &'static str {
    match level {
        "DEBUG" => "DBG",
        "WARN" => "WRN",
        _ => "INF",
    }
}

/// A moment, as milliseconds after 2026-11-11 00:00:00.
#[derive(Clone, Copy)]
struct Time(u64);

impl Time {
    fn clock(self) -> String {
        let s = self.0 / 1000 % 86_400;
        format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }

    fn millis(self) -> u64 {
        self.0 % 1000
    }

    /// `2026-11-11T09:00:01.234Z`
    fn stamp(self) -> String {
        let (y, m, d) = civil(EPOCH_DAY + (self.0 / 86_400_000) as i64);
        format!(
            "{y:04}-{m:02}-{d:02}T{}.{:03}Z",
            self.clock(),
            self.millis()
        )
    }
}

/// Log-line prefix sized to the pane: full ISO (~43 cols), compact (~26) or tiny (13).
fn prefix_segments(t: Time, level: &str, svc: &str, width: usize) -> Vec<Segment> {
    let lvl = level_style(level);
    if width >= FULL_MIN_WIDTH {
        return vec![
            seg(format!("{} ", t.stamp()), DIM),
            seg(format!("{level:<5} "), lvl),
            seg(format!("{svc:<SVC_WIDTH$}  "), CYAN),
        ];
    }
    if width >= COMPACT_MIN_WIDTH {
        let module = svc.trim_start_matches("svc.");
        return vec![
            seg(format!("{}.{:03} ", t.clock(), t.millis()), DIM),
            seg(format!("{level:<5} "), lvl),
            seg(format!("{module:<6} "), CYAN),
        ];
    }
    vec![
        seg(format!("{} ", t.clock()), DIM),
        seg(format!("{} ", short_level(level)), lvl),
    ]
}

fn prefix_len(width: usize) -> usize {
    prefix_segments(Time(EPOCH_MS), "INFO", BOOK_SVC, width)
        .iter()
        .map(|(t, _)| t.width())
        .sum()
}

pub struct LogMask {
    /// Time of the last line: filler continues from it.
    t: u64,
}

impl Default for LogMask {
    fn default() -> Self {
        LogMask { t: EPOCH_MS }
    }
}

impl LogMask {
    fn tick(&mut self, rng: &mut Rng) -> Time {
        self.t += randint(rng, 20, MAX_STEP_MS as usize) as u64;
        Time(self.t)
    }

    fn entry(&self, rng: &mut Rng, t: Time, width: usize) -> Line {
        let level = *weighted(rng, LEVELS, &[6, 2, 1]);
        let mut segs = prefix_segments(t, level, pick(rng, SERVICES), width);
        segs.push(seg(*pick(rng, messages(level)), FG));
        chrome_owned(width, segs)
    }

    fn body(width: usize) -> usize {
        width.saturating_sub(prefix_len(width)).max(1)
    }
}

impl Mask for LogMask {
    fn name(&self) -> &'static str {
        "log"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        let mut text = "-- Logs begin at Wed 2026-11-11 09:00:00 UTC --";
        if text.width() > width {
            text = "-- Logs begin at 2026-11-11 09:00:00 --";
        }
        vec![chrome(width, &[(text, DIM)])]
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        let t = self.tick(rng);
        self.entry(rng, t, width)
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged(&para.text, Self::body(width)).len() + 2
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        self.t = EPOCH_MS + line_no_start as u64 * LINE_MS;
        let t = self.tick(rng);
        let mut out = vec![self.entry(rng, t, width)];
        for text in ragged(&para.text, Self::body(width)) {
            let mut segs = prefix_segments(self.tick(rng), "INFO", BOOK_SVC, width);
            segs.push(seg(text, format!("book {FG}")));
            out.push(Line::book(segs, pos));
        }
        let t = self.tick(rng);
        out.push(self.entry(rng, t, width));
        out
    }

    /// The worker moving on to a new batch, or its log being rotated.
    fn transition(&self, chapter: usize, width: usize, line_no_start: usize) -> Vec<Line> {
        // on the line's own time slot: after the paragraph above, before the one below
        let t = Time(EPOCH_MS + line_no_start as u64 * LINE_MS);
        let name = section::name(chapter, 0);
        let n = chapter + 1;
        let text = match chapter % 4 {
            0 => format!("starting batch {n} queue={name}"),
            1 => format!("log rotated: worker.log.{n} -> worker.log.{}", n + 1),
            2 => format!("picked up job set {name} (batch {n})"),
            _ => format!("consumer rebalanced: partition={n} topic={name}"),
        };
        let mut segs = prefix_segments(t, "INFO", BOOK_SVC, width);
        segs.push(seg(text, format!("bold {FG}")));
        vec![chrome_owned(width, segs)]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Tailing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para(n: usize) -> Paragraph {
        Paragraph::new("Oda dar ve basıktı, tavanı alçaktı. ".repeat(n))
    }

    fn long() -> Paragraph {
        Paragraph::new(
            "Oda dar ve basıktı, tavanı alçaktı; pencereden giren ışık azdı. ".repeat(12),
        )
    }

    fn rng(seed: u64) -> Rng {
        Rng::seed_from_u64(seed)
    }

    /// The timestamp at the start of a full-width line, as sortable text.
    fn stamp(line: &str) -> &str {
        assert!(line.len() > 24 && &line[23..25] == "Z ", "{line}");
        &line[..23]
    }

    // -- test_log.py --

    #[test]
    fn every_line_matches_format_and_timestamps_increase() {
        let lines = LogMask::default().render(&para(6), (0, 0), 120, &mut rng(1), 41);
        assert!(lines.len() > 3);
        let times: Vec<String> = lines
            .iter()
            .map(|l| stamp(&l.plain()).to_string())
            .collect();
        assert!(times.windows(2).all(|w| w[0] < w[1]), "{times:?}");
        assert!(lines.iter().all(|l| {
            ["INFO ", "DEBUG", "WARN "]
                .iter()
                .any(|lv| l.plain()[25..].starts_with(lv))
        }));
    }

    #[test]
    fn book_lines_are_info_and_styled_as_book() {
        let lines = LogMask::default().render(&para(6), (0, 0), 120, &mut rng(1), 1);
        let book: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(!book.is_empty());
        assert!(
            book.iter()
                .all(|l| l.plain().contains(" INFO  svc.worker  "))
        );
        assert!(
            book.iter()
                .all(|l| l.segments.last().unwrap().1 == format!("book {FG}"))
        );
    }

    #[test]
    fn render_is_deterministic_and_depends_on_line_no_start() {
        let mut m = LogMask::default();
        let plain = |m: &mut LogMask, start| -> Vec<String> {
            m.render(&para(6), (0, 0), 90, &mut rng(1), start)
                .iter()
                .map(Line::plain)
                .collect()
        };
        let a = plain(&mut m, 5);
        assert_eq!(a, plain(&mut m, 5));
        assert_ne!(a, plain(&mut m, 400));
    }

    #[test]
    fn noise_levels_vary_and_have_their_own_colours() {
        let mut m = LogMask::default();
        let mut r = rng(3);
        let mut by_level: std::collections::HashMap<String, std::collections::HashSet<String>> =
            Default::default();
        for _ in 0..200 {
            let l = m.noise(&mut r, 120);
            assert!(!l.is_book);
            by_level
                .entry(l.segments[1].0.trim().to_string())
                .or_default()
                .insert(l.segments[1].1.clone());
        }
        assert_eq!(by_level.len(), 3);
        assert!(by_level.values().all(|s| s.len() == 1));
        let colours: std::collections::HashSet<&String> = by_level
            .values()
            .map(|s| s.iter().next().unwrap())
            .collect();
        assert_eq!(colours.len(), 3);
    }

    #[test]
    fn progress_label() {
        assert_eq!(
            LogMask::default().progress_label(142, 380, 50),
            "Tailing [===>      ] 142/380"
        );
    }

    #[test]
    fn prefix_adapts_to_width_and_every_line_fits() {
        for width in [40, 60, 120] {
            let mut m = LogMask::default();
            let mut r = rng(5);
            let mut lines = m.render(&long(), (0, 0), width, &mut r, 7);
            lines.extend((0..100).map(|_| m.noise(&mut r, width)));
            for l in &lines {
                let p = l.plain();
                assert!(p.chars().count() <= width, "{p}");
                let ok = match width {
                    120 => p.len() > 24 && &p[23..25] == "Z ",
                    60 => {
                        p.as_bytes()[2] == b':'
                            && p.as_bytes()[8] == b'.'
                            && p.as_bytes()[12] == b' '
                    }
                    _ => p.as_bytes()[8] == b' ' && ["INF ", "DBG ", "WRN "].contains(&&p[9..13]),
                };
                assert!(ok, "{width}: {p}");
            }
            let book: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
            let body: Vec<&str> = book
                .iter()
                .map(|l| l.segments.last().unwrap().0.as_str())
                .collect();
            let prefix = book[0].plain().chars().count() - body[0].chars().count();
            assert!(width - prefix >= 20);
            assert_eq!(
                body.join(" ").split_whitespace().collect::<Vec<_>>(),
                long().text.split_whitespace().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn compact_timestamps_increase() {
        for width in [40, 60] {
            let lines = LogMask::default().render(&long(), (0, 0), width, &mut rng(1), 41);
            let stamps: Vec<String> = lines
                .iter()
                .map(|l| l.plain().split(' ').next().unwrap().to_string())
                .collect();
            let mut sorted = stamps.clone();
            sorted.sort();
            assert_eq!(stamps, sorted);
            assert!(
                stamps
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    > 1
            );
        }
    }

    #[test]
    fn dates_roll_over_days_months_and_years() {
        assert_eq!(Time(EPOCH_MS + 1234).stamp(), "2026-11-11T09:00:01.234Z");
        assert_eq!(
            Time(86_400_000 * 20 + 5).stamp(),
            "2026-12-01T00:00:00.005Z"
        );
        assert_eq!(Time(86_400_000 * 51).stamp(), "2027-01-01T00:00:00.000Z");
    }

    #[test]
    fn long_text_is_in_the_plain_foreground_colour() {
        let lines = LogMask::default().render(&para(6), (0, 0), 80, &mut rng(1), 1);
        assert!(
            lines
                .iter()
                .filter(|l| l.is_book)
                .all(|l| l.segments.last().unwrap().1 == format!("book {FG}"))
        );
    }
}
