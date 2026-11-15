//! Reading statistics: words and seconds per day and per text, kept in
//! `stats.json` next to the progress file.
//!
//! Texts are keyed by their id only: no file name and no text of any book is
//! ever stored. Days are counted from 1970-01-01 in the local calendar; the
//! caller passes the day in, so nothing here reads the clock.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::masks::time::{civil, days_from_civil};

/// Words a printed page holds, for the "pages" figure.
pub const WORDS_PER_PAGE: u64 = 250;
/// Days the sparkline covers.
pub const SPARK_DAYS: usize = 14;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals {
    pub words: u64,
    pub seconds: f64,
}

impl Totals {
    fn add(&mut self, words: u64, seconds: f64) {
        self.words += words;
        self.seconds += seconds;
    }

    pub fn pages(&self) -> u64 {
        (self.words + WORDS_PER_PAGE / 2) / WORDS_PER_PAGE
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    days: BTreeMap<i64, Totals>,
    books: BTreeMap<String, Totals>,
}

/// `2026-11-15` for a day number.
pub fn date_text(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("{y:04}-{m:02}-{d:02}")
}

fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let day = days_from_civil(y, m, d);
    (civil(day) == (y, m, d)).then_some(day)
}

fn totals(value: &Value) -> Option<Totals> {
    let get = |key: &str| {
        value
            .get(key)?
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0)
    };
    Some(Totals {
        words: get("words")? as u64,
        seconds: get("seconds")?,
    })
}

fn entry(t: &Totals) -> Value {
    json!({ "words": t.words, "seconds": (t.seconds * 10.0).round() / 10.0 })
}

impl Stats {
    /// A saved file's contents; entries that make no sense are skipped.
    pub fn from_object(obj: &Map<String, Value>) -> Stats {
        let mut stats = Stats::default();
        if let Some(Value::Object(days)) = obj.get("days") {
            for (date, v) in days {
                if let (Some(day), Some(t)) = (parse_date(date), totals(v)) {
                    stats.days.insert(day, t);
                }
            }
        }
        if let Some(Value::Object(books)) = obj.get("books") {
            for (id, v) in books {
                if let Some(t) = totals(v) {
                    stats.books.insert(id.clone(), t);
                }
            }
        }
        stats
    }

    pub fn to_json(&self) -> Value {
        json!({
            "version": 1,
            "days": self.days.iter().map(|(d, t)| (date_text(*d), entry(t))).collect::<Map<_, _>>(),
            "books": self.books.iter().map(|(id, t)| (id.clone(), entry(t))).collect::<Map<_, _>>(),
        })
    }

    /// Words read and seconds spent on text `book` on `day`.
    pub fn record(&mut self, day: i64, book: &str, words: usize, seconds: f64) {
        if words == 0 || !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        self.days.entry(day).or_default().add(words as u64, seconds);
        self.books
            .entry(book.to_string())
            .or_default()
            .add(words as u64, seconds);
    }

    pub fn is_empty(&self) -> bool {
        self.days.is_empty()
    }

    pub fn day(&self, day: i64) -> Totals {
        self.days.get(&day).copied().unwrap_or_default()
    }

    pub fn book(&self, id: &str) -> Totals {
        self.books.get(id).copied().unwrap_or_default()
    }

    /// The sum over the `n` days ending on `today`.
    pub fn last(&self, today: i64, n: usize) -> Totals {
        let mut sum = Totals::default();
        for (_, t) in self.days.range(today - n as i64 + 1..=today) {
            sum.add(t.words, t.seconds);
        }
        sum
    }

    /// Words per day for the `n` days ending on `today`, oldest first.
    pub fn daily_words(&self, today: i64, n: usize) -> Vec<u64> {
        (0..n as i64)
            .map(|i| self.day(today - n as i64 + 1 + i).words)
            .collect()
    }

    /// Days in a row with reading, up to today (a day not yet read in doesn't break it).
    pub fn streak(&self, today: i64) -> u32 {
        let reads = |d: i64| self.day(d).words > 0;
        let mut day = if reads(today) { today } else { today - 1 };
        let mut n = 0;
        while reads(day) {
            n += 1;
            day -= 1;
        }
        n
    }

    /// Words per minute over everything read, once there is a minute of it.
    pub fn average_wpm(&self) -> Option<u32> {
        let all = self.days.values().fold(Totals::default(), |mut a, t| {
            (a.add(t.words, t.seconds), a).1
        });
        (all.seconds >= 60.0).then(|| (all.words as f64 * 60.0 / all.seconds).round() as u32)
    }
}

/// Seconds to read `words` words at `wpm`.
pub fn eta_seconds(words: u64, wpm: u32) -> u64 {
    words * 60 / u64::from(wpm.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 20_772; // 2026-11-15

    #[test]
    fn records_add_up_per_day_and_per_text() {
        let mut s = Stats::default();
        s.record(DAY, "a", 100, 30.0);
        s.record(DAY, "b", 50, 10.0);
        s.record(DAY + 1, "a", 20, 5.0);
        assert_eq!(s.day(DAY).words, 150);
        assert_eq!(s.day(DAY + 1).words, 20);
        assert_eq!(s.book("a").words, 120);
        assert_eq!(s.book("a").seconds, 35.0);
        assert_eq!(s.last(DAY + 1, 2).words, 170);
        assert_eq!(s.last(DAY + 1, 1).words, 20);
        s.record(DAY, "a", 0, 9.0); // nothing read, nothing recorded
        assert_eq!(s.day(DAY).seconds, 40.0);
    }

    #[test]
    fn streak_counts_back_from_today_or_yesterday() {
        let mut s = Stats::default();
        for d in [DAY - 3, DAY - 2, DAY - 1, DAY] {
            s.record(d, "a", 10, 1.0);
        }
        s.record(DAY - 5, "a", 10, 1.0);
        assert_eq!(s.streak(DAY), 4);
        assert_eq!(s.streak(DAY + 1), 4); // today still to come
        assert_eq!(s.streak(DAY + 2), 0);
    }

    #[test]
    fn daily_words_run_oldest_first() {
        let mut s = Stats::default();
        s.record(DAY, "a", 7, 1.0);
        s.record(DAY - 2, "a", 3, 1.0);
        assert_eq!(s.daily_words(DAY, 3), vec![3, 0, 7]);
    }

    #[test]
    fn average_rate_needs_a_minute_of_reading() {
        let mut s = Stats::default();
        s.record(DAY, "a", 100, 30.0);
        assert_eq!(s.average_wpm(), None);
        s.record(DAY, "a", 300, 90.0);
        assert_eq!(s.average_wpm(), Some(200));
    }

    #[test]
    fn eta_is_remaining_words_over_the_rate() {
        assert_eq!(eta_seconds(2_000, 200), 600);
        assert_eq!(eta_seconds(0, 200), 0);
        assert_eq!(eta_seconds(100, 0), 6_000); // a zero rate doesn't divide by zero
    }

    #[test]
    fn json_round_trip_keeps_days_and_texts() {
        let mut s = Stats::default();
        s.record(DAY, "abc", 120, 61.5);
        s.record(DAY - 40, "def", 9, 3.0);
        let json = s.to_json();
        assert_eq!(json["days"]["2026-11-15"]["words"], 120);
        let back = Stats::from_object(json.as_object().unwrap());
        assert_eq!(back, s);
    }

    #[test]
    fn nonsense_entries_are_skipped() {
        let v = json!({
            "days": { "2026-02-30": {"words": 1, "seconds": 1}, "x": {"words": 1, "seconds": 1},
                      "2026-11-15": {"words": -4, "seconds": 1}, "2026-11-14": {"words": 5, "seconds": 2} },
            "books": { "a": "text", "b": {"words": 1, "seconds": 2} },
        });
        let s = Stats::from_object(v.as_object().unwrap());
        assert_eq!(s.days.len(), 1);
        assert_eq!(s.day(DAY - 1).words, 5);
        assert_eq!(s.books.len(), 1);
    }
}
