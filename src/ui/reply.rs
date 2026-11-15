//! A fake reply to a request typed into the input box, played out on a
//! timer: the mask's groups of lines (the request, a thinking moment, tool
//! calls, a closing remark) appear a line at a time, with pauses between
//! groups, then stay up a moment before the screen goes back to the text.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::masks::Line;

/// Between the lines of one group.
pub const LINE_GAP: Duration = Duration::from_millis(70);
/// Before the thinking moment, right after the request is echoed.
pub const ECHO_GAP: Duration = Duration::from_millis(300);
/// The thinking moment, before the first tool call.
pub const THINK: Duration = Duration::from_millis(1500);
/// Between tool calls.
pub const GROUP_GAP: Duration = Duration::from_millis(600);
/// The finished reply stays up this long.
pub const HOLD: Duration = Duration::from_millis(2500);

pub struct Reply {
    /// Lines still to come, each with the wait before it.
    steps: VecDeque<(Duration, Line)>,
    /// When the newest line came up.
    last: Instant,
}

impl Reply {
    /// Play `groups` from `now`: the first group's first line is due at once.
    pub fn new(groups: Vec<Vec<Line>>, now: Instant) -> Reply {
        let mut steps = VecDeque::new();
        let mut gap = Duration::ZERO;
        for (g, group) in groups.into_iter().enumerate() {
            for (i, line) in group.into_iter().enumerate() {
                steps.push_back((if i == 0 { gap } else { LINE_GAP }, line));
            }
            gap = match g {
                0 => ECHO_GAP,
                1 => THINK,
                _ => GROUP_GAP,
            };
        }
        Reply { steps, last: now }
    }

    /// When the next line is due, or the reply is over.
    pub fn due(&self) -> Instant {
        match self.steps.front() {
            Some((wait, _)) => self.last + *wait,
            None => self.last + HOLD,
        }
    }

    /// The lines due by `now`, and whether the reply is over.
    pub fn take_due(&mut self, now: Instant) -> (Vec<Line>, bool) {
        let mut out = vec![];
        while let Some((wait, _)) = self.steps.front() {
            if self.last + *wait > now {
                return (out, false);
            }
            self.last += *wait;
            out.push(self.steps.pop_front().expect("checked above").1);
        }
        (out, now >= self.last + HOLD)
    }

    /// How long the whole reply takes, the hold at the end included.
    pub fn length(&self) -> Duration {
        self.steps.iter().map(|(w, _)| *w).sum::<Duration>() + HOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks::base::plain_line;
    use crate::masks::palette::FG;

    fn group(n: usize) -> Vec<Line> {
        (0..n).map(|i| plain_line(format!("l{i}"), FG)).collect()
    }

    #[test]
    fn lines_come_on_time_then_hold_then_end() {
        let t0 = Instant::now();
        let mut r = Reply::new(vec![group(2), group(1), group(2)], t0);
        assert_eq!(r.length(), LINE_GAP * 2 + ECHO_GAP + THINK + HOLD);
        let (first, done) = r.take_due(t0);
        assert_eq!((first.len(), done), (1, false));
        assert_eq!(r.due(), t0 + LINE_GAP);
        assert_eq!(r.take_due(t0 + LINE_GAP).0.len(), 1);
        assert_eq!(r.take_due(t0 + LINE_GAP + ECHO_GAP).0.len(), 1);
        let end = t0 + LINE_GAP * 2 + ECHO_GAP + THINK;
        assert_eq!(r.take_due(end).0.len(), 2, "late: every due line at once");
        assert_eq!(r.due(), end + HOLD);
        assert!(!r.take_due(end + HOLD / 2).1);
        assert!(r.take_due(end + HOLD).1);
    }
}
