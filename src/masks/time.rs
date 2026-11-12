//! Calendar arithmetic for timestamps in the masks, without a date crate.

/// Days from 1970-01-01 to 2026-11-11, the day the masks' clocks start on.
pub const EPOCH_DAY: i64 = 20_768;

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
    fn dates_and_weekdays() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(EPOCH_DAY), (2026, 11, 11));
        assert_eq!(weekday(EPOCH_DAY), "Wed");
        assert_eq!(civil(EPOCH_DAY + 51), (2027, 1, 1));
        assert_eq!(civil(EPOCH_DAY - 315), (2025, 12, 31));
        assert_eq!(month(11), "Nov");
    }
}
