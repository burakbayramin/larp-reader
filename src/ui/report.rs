//! The statistics screen: today, the week, the streak, the rate and the time
//! left, drawn as the current mask's tool would print a report.
//!
//! The screen names no text and no file: numbers only. Words are "words",
//! screens are "pages".

use ratatui::Frame;
use ratatui::text::{Line as TLine, Span};
use ratatui::widgets::{Block, Paragraph};

use super::theme::Theme;
use crate::masks::base::seg;
use crate::masks::palette::{CYAN, DIM, FG, GREEN, YELLOW};
use crate::masks::time::{month, weekday};
use crate::masks::{Line, Segment};
use crate::stats::{SPARK_DAYS, Stats, Totals, date_text, eta_seconds};

const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// What the report shows, worked out when it opens.
pub struct Report {
    pub day: i64,
    pub today: Totals,
    pub week: Totals,
    pub streak: u32,
    /// Words per minute, and whether it was measured (else it is the setting).
    pub wpm: u32,
    pub measured: bool,
    /// Seconds of reading left in the open text.
    pub eta: Option<u64>,
    /// Words per day, oldest first, ending today.
    pub daily: Vec<u64>,
}

impl Report {
    /// `remaining` is the words left in the open text, if one is open.
    pub fn new(stats: &Stats, day: i64, setting_wpm: u32, remaining: Option<u64>) -> Report {
        let measured = stats.average_wpm();
        let wpm = measured.unwrap_or(setting_wpm);
        Report {
            day,
            today: stats.day(day),
            week: stats.last(day, 7),
            streak: stats.streak(day),
            wpm,
            measured: measured.is_some(),
            eta: remaining.map(|w| eta_seconds(w, wpm)),
            daily: stats.daily_words(day, SPARK_DAYS),
        }
    }

    fn spark(&self) -> String {
        let top = self.daily.iter().copied().max().unwrap_or(0);
        self.daily
            .iter()
            .map(|&w| match (w, top) {
                (0, _) => SPARK[0],
                (w, top) => SPARK[(1 + w * 6 / top.max(1)).min(7) as usize],
            })
            .collect()
    }

    /// The report as lines in the style of the mask called `mask`.
    pub fn lines(&self, mask: &str) -> Vec<Line> {
        match mask {
            "test" => self.pytest(),
            "log" => self.log(),
            "git" => self.git(),
            _ => self.table(mask),
        }
    }

    // -- one look per tool -----------------------------------------------------------

    /// A metrics table; the heading is the mask's own.
    fn table(&self, mask: &str) -> Vec<Line> {
        let head = match mask {
            "agent" => vec![seg("✻ ", CYAN), seg("Session metrics", FG.bold())],
            "build" => vec![
                seg("    Finished ", GREEN.bold()),
                seg("`bench` profile [optimized] target(s)", FG),
            ],
            "docker" => vec![seg("$ ", DIM), seg("docker stats --no-stream", FG)],
            "diff" => vec![seg("$ ", DIM), seg("git diff --shortstat", FG)],
            _ => vec![seg("$ ", DIM), seg("python -m metrics --summary", FG)],
        };
        let row = |label: &str, value: Vec<Segment>| {
            let mut v = vec![seg(format!("  {label:<12}"), DIM)];
            v.extend(value);
            line(v)
        };
        let mut out = vec![line(head), line(vec![])];
        out.push(row("today", self.span(&self.today)));
        out.push(row("this week", self.span(&self.week)));
        out.push(row(
            "streak",
            vec![seg(self.streak.to_string(), YELLOW), seg(" days", FG)],
        ));
        out.push(row("throughput", self.rate()));
        out.push(row("ETA", self.eta_value()));
        out.push(line(vec![]));
        out.push(row(
            &format!("last {SPARK_DAYS}d"),
            vec![seg(self.spark(), GREEN)],
        ));
        out.push(line(vec![]));
        out.push(line(vec![seg("(END)", DIM)]));
        out
    }

    fn pytest(&self) -> Vec<Line> {
        let dots: Vec<Segment> = self
            .daily
            .iter()
            .map(|&w| {
                if w > 0 {
                    seg(".", GREEN)
                } else {
                    seg("s", YELLOW)
                }
            })
            .collect();
        let skipped = self.daily.iter().filter(|&&w| w == 0).count();
        let passed = self.daily.len() - skipped;
        let mut run = vec![seg("reading/test_daily.py ", FG)];
        run.extend(dots);
        run.push(seg(format!("{:>36}", "[100%]"), DIM));
        let slow = |time: String, kind: &str, name: &str, note: String| {
            line(vec![
                seg(format!("{time:>9}"), YELLOW),
                seg(format!(" {kind:<9}"), DIM),
                seg(name, FG),
                seg(format!("  ({note})"), DIM),
            ])
        };
        vec![
            line(vec![seg(
                "============================= test session starts =============================",
                FG.bold(),
            )]),
            line(vec![seg(
                format!("collected {} items", self.daily.len()),
                FG,
            )]),
            line(vec![]),
            line(run),
            line(vec![]),
            line(vec![seg(
                "========================== slowest durations ==========================",
                FG.bold(),
            )]),
            slow(
                duration(self.today.seconds),
                "call",
                "test_today",
                self.span_text(&self.today),
            ),
            slow(
                duration(self.week.seconds),
                "call",
                "test_week",
                self.span_text(&self.week),
            ),
            slow(
                self.eta.map_or("-".into(), eta_text),
                "teardown",
                "test_eta",
                "remaining at current pace".into(),
            ),
            slow(
                format!("{} wpm", compact(self.wpm as u64)),
                "setup",
                "test_throughput",
                self.rate_text(),
            ),
            slow(
                format!("{} d", self.streak),
                "call",
                "test_streak",
                "consecutive days".into(),
            ),
            line(vec![]),
            line(vec![
                seg("=================== ", GREEN),
                seg(format!("{passed} passed"), GREEN.bold()),
                seg(format!(", {skipped} skipped"), YELLOW),
                seg(format!(" in {}", duration(self.week.seconds)), GREEN),
                seg(" ===================", GREEN),
            ]),
        ]
    }

    fn log(&self) -> Vec<Line> {
        let stamp = format!("{} 23:59:59", date_text(self.day));
        let row = |level: &str, what: &str, fields: Vec<Segment>| {
            let mut v = vec![
                seg(format!("{stamp} "), DIM),
                seg(format!("{level:<5} "), GREEN),
                seg(format!("{what:<8} "), CYAN),
            ];
            v.extend(fields);
            line(v)
        };
        let kv = |k: &str, v: String| vec![seg(format!("{k}="), DIM), seg(format!("{v} "), FG)];
        let totals = |t: &Totals| {
            let mut f = kv("pages", t.pages().to_string());
            f.extend(kv("words", compact(t.words)));
            f.extend(kv("elapsed", duration(t.seconds)));
            f
        };
        let mut out = vec![
            row("INFO", "metrics", vec![seg("report start", FG)]),
            row("INFO", "today", totals(&self.today)),
            row("INFO", "week", totals(&self.week)),
            row("INFO", "streak", kv("days", self.streak.to_string())),
            row(
                "INFO",
                "rate",
                [
                    kv("wpm", self.wpm.to_string()),
                    kv("src", if self.measured { "avg" } else { "cfg" }.into()),
                ]
                .concat(),
            ),
        ];
        if let Some(eta) = self.eta {
            out.push(row("INFO", "eta", kv("remaining", eta_text(eta))));
        }
        out.push(row(
            "DEBUG",
            "daily",
            vec![
                seg(format!("last{SPARK_DAYS}="), DIM),
                seg(self.spark(), GREEN),
            ],
        ));
        out.push(row("INFO", "metrics", vec![seg("report done", FG)]));
        out
    }

    fn git(&self) -> Vec<Line> {
        let top = self.daily.iter().copied().max().unwrap_or(0).max(1);
        let mut out = vec![
            line(vec![
                seg("$ ", DIM),
                seg("git log --stat --since=\"14 days ago\"", FG),
            ]),
            line(vec![]),
        ];
        for (i, &w) in self.daily.iter().enumerate() {
            let d = self.day - (SPARK_DAYS - 1 - i) as i64;
            let (_, m, dd) = crate::masks::time::civil(d);
            let bar = "+".repeat((w * 30 / top) as usize + usize::from(w > 0));
            out.push(line(vec![
                seg(format!(" {} {} {dd:>2} ", weekday(d), month(m)), FG),
                seg("| ", DIM),
                seg(format!("{:>5} ", compact(w)), YELLOW),
                seg(bar, GREEN),
            ]));
        }
        let pages = self.week.pages();
        out.push(line(vec![seg(
            format!(
                " {SPARK_DAYS} days changed, {} insertions(+), {} pages",
                compact(self.daily.iter().sum()),
                pages
            ),
            FG,
        )]));
        out.push(line(vec![]));
        out.push(line(vec![
            seg(" streak ", DIM),
            seg(format!("{} days", self.streak), YELLOW),
            seg("  ·  rate ", DIM),
            seg(self.rate_text(), FG),
            seg("  ·  eta ", DIM),
            seg(self.eta.map_or("-".into(), eta_text), FG),
        ]));
        out
    }

    // -- pieces -----------------------------------------------------------------------

    fn span_text(&self, t: &Totals) -> String {
        format!(
            "{} pages  ·  {} words  ·  {}",
            t.pages(),
            compact(t.words),
            duration(t.seconds)
        )
    }

    fn span(&self, t: &Totals) -> Vec<Segment> {
        vec![
            seg(t.pages().to_string(), YELLOW),
            seg(" pages", FG),
            seg("  ·  ", DIM),
            seg(compact(t.words), CYAN),
            seg(" words", FG),
            seg("  ·  ", DIM),
            seg(duration(t.seconds), GREEN),
        ]
    }

    fn rate_text(&self) -> String {
        format!(
            "{} wpm ({})",
            compact(self.wpm as u64),
            if self.measured { "avg" } else { "est" }
        )
    }

    fn rate(&self) -> Vec<Segment> {
        vec![
            seg(compact(self.wpm as u64), CYAN),
            seg(
                format!(" wpm ({})", if self.measured { "avg" } else { "est" }),
                FG,
            ),
        ]
    }

    fn eta_value(&self) -> Vec<Segment> {
        match self.eta {
            Some(0) => vec![seg("done", GREEN)],
            Some(s) => vec![seg(eta_text(s), YELLOW), seg(" at current pace", FG)],
            None => vec![seg("-", DIM)],
        }
    }

    /// Paint the report over the whole screen.
    pub fn draw(&self, mask: &str, frame: &mut Frame, theme: &Theme) {
        frame.render_widget(Block::default().style(theme.base()), frame.area());
        let text: Vec<TLine> = self
            .lines(mask)
            .iter()
            .map(|l| {
                TLine::from(
                    l.segments
                        .iter()
                        .map(|(t, look)| Span::styled(t.clone(), theme.style(*look)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(text), frame.area());
    }
}

fn line(segments: Vec<Segment>) -> Line {
    Line {
        segments,
        is_book: false,
        pos: None,
    }
}

/// `950`, `9.8k`, `12k`.
pub fn compact(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=9_999 => format!("{:.1}k", n as f64 / 1000.0),
        _ => format!("{}k", n / 1000),
    }
}

/// `38m`, `1h 05m`.
pub fn duration(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as u64;
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    }
}

/// `9m`, `2h 14m`, `3d 4h`: the time a stretch of reading takes.
pub fn eta_text(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    match minutes {
        0..=59 => format!("{minutes}m"),
        60..=1439 => format!("{}h {:02}m", minutes / 60, minutes % 60),
        _ => format!("{}d {}h", minutes / 1440, minutes % 1440 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eta_is_the_words_left_at_the_measured_rate() {
        let mut stats = Stats::default();
        stats.record(20_772, "a", 600, 180.0); // 200 wpm
        let r = Report::new(&stats, 20_772, 300, Some(2_000));
        assert!(r.measured);
        assert_eq!((r.wpm, r.eta), (200, Some(600)));
        let r = Report::new(&Stats::default(), 20_772, 300, Some(3_000));
        assert!(!r.measured); // nothing read yet: the setting stands in
        assert_eq!(r.eta, Some(600));
        assert_eq!(Report::new(&stats, 20_772, 300, None).eta, None);
    }

    #[test]
    fn the_sparkline_scales_to_the_busiest_day() {
        let mut stats = Stats::default();
        stats.record(20_772, "a", 600, 60.0);
        stats.record(20_771, "a", 100, 60.0);
        let spark = Report::new(&stats, 20_772, 200, None).spark();
        assert_eq!(spark.chars().count(), SPARK_DAYS);
        assert!(spark.ends_with("▃█"), "{spark}");
        assert!(spark.starts_with('▁'));
    }

    #[test]
    fn numbers_read_compactly() {
        assert_eq!(compact(950), "950");
        assert_eq!(compact(9_800), "9.8k");
        assert_eq!(compact(12_400), "12k");
        assert_eq!(duration(38.0 * 60.0), "38m");
        assert_eq!(duration(3_900.0), "1h 05m");
        assert_eq!(eta_text(540), "9m");
        assert_eq!(eta_text(2 * 3600 + 14 * 60), "2h 14m");
        assert_eq!(eta_text(3 * 86_400 + 4 * 3600 + 120), "3d 4h");
    }
}
