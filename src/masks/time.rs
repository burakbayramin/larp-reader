//! Calendar arithmetic for timestamps in the masks, without a date crate.

/// Days from 1970-01-01 to 2026-11-11, the day the masks' clocks start on in tests.
#[cfg(test)]
pub const EPOCH_DAY: i64 = 20_768;

/// The moment the masks' clocks start at: taken once per process, so a
/// paragraph renders identically on every draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    /// Unix milliseconds (UTC).
    pub utc_ms: i64,
    /// The machine's offset from UTC, in minutes.
    pub offset_min: i64,
}

impl Start {
    /// Unix minutes of this moment, shifted into the local zone.
    pub fn local_minutes(self) -> i64 {
        self.utc_ms.div_euclid(60_000) + self.offset_min
    }
}

/// `+0300` / `-0530`.
pub fn offset_text(offset_min: i64) -> String {
    let sign = if offset_min < 0 { '-' } else { '+' };
    let a = offset_min.abs();
    format!("{sign}{:02}{:02}", a / 60, a % 60)
}

/// The real current moment and the machine's UTC offset.
fn start_now() -> Start {
    let now = chrono::Local::now();
    Start {
        utc_ms: now.timestamp_millis(),
        offset_min: i64::from(now.offset().local_minus_utc()) / 60,
    }
}

/// The session's start. Tests get a fixed moment (2026-11-11 09:00 UTC,
/// +0300) so the golden screens stay put.
pub fn session_start() -> Start {
    static START: std::sync::OnceLock<Start> = std::sync::OnceLock::new();
    *START.get_or_init(|| {
        #[cfg(test)]
        {
            Start {
                utc_ms: (EPOCH_DAY * 86_400 + 9 * 3600) * 1000,
                offset_min: 180,
            }
        }
        #[cfg(not(test))]
        {
            start_now()
        }
    })
}

/// (year, month, day) of a day counted from 1970-01-01 (Howard Hinnant's civil_from_days).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Days from 1970-01-01 of a calendar date (the inverse of `civil`).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The local date's day number now, in the machine's own time zone (on
/// every platform; UTC only where the zone can't be told).
pub fn local_today() -> i64 {
    let now = chrono::Local::now();
    let secs = now.timestamp();
    let offset = i64::from(now.offset().local_minus_utc());
    (secs + offset).div_euclid(86_400)
}

/// `Wed`, `Thu` … (1970-01-01 was a Thursday).
pub fn weekday(days: i64) -> &'static str {
    ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][days.rem_euclid(7) as usize]
}

pub fn month(m: u32) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m - 1) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_today_is_the_local_calendar_date() {
        let date = chrono::Local::now().date_naive();
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        let want = (date - epoch).num_days();
        let got = local_today();
        // the date may roll over between the two reads, right at midnight
        assert!(got == want || got == want + 1, "{got} vs {want}");
    }

    #[test]
    fn the_real_start_is_the_current_moment() {
        let s = start_now();
        let real = chrono::Utc::now().timestamp_millis();
        assert!((real - s.utc_ms).abs() < 60_000, "{} vs {real}", s.utc_ms);
        assert!(s.offset_min.abs() <= 14 * 60);
        // local minutes are the wall clock
        let local = chrono::Local::now().naive_local();
        let want = local.and_utc().timestamp().div_euclid(60);
        assert!((s.local_minutes() - want).abs() <= 1);
    }

    #[test]
    fn the_session_start_is_taken_once() {
        assert_eq!(session_start(), session_start());
    }

    #[test]
    fn offsets_print_like_git() {
        assert_eq!(offset_text(180), "+0300");
        assert_eq!(offset_text(-330), "-0530");
        assert_eq!(offset_text(0), "+0000");
    }

    #[test]
    fn local_minutes_apply_the_offset() {
        let s = Start {
            utc_ms: 120_000,
            offset_min: 60,
        };
        assert_eq!(s.local_minutes(), 62);
    }

    #[test]
    fn dates_and_weekdays() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(EPOCH_DAY), (2026, 11, 11));
        assert_eq!(weekday(EPOCH_DAY), "Wed");
        assert_eq!(civil(EPOCH_DAY + 51), (2027, 1, 1));
        assert_eq!(civil(EPOCH_DAY - 315), (2025, 12, 31));
        assert_eq!(month(11), "Nov");
        for d in [0, EPOCH_DAY, EPOCH_DAY + 51, EPOCH_DAY - 315, -800] {
            let (y, m, dd) = civil(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }
}
