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

/// The local date's day number now (UTC where the zone can't be told).
pub fn local_today() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    (secs + utc_offset(secs)).div_euclid(86_400)
}

#[cfg(unix)]
fn utc_offset(secs: i64) -> i64 {
    let t = secs as libc::time_t;
    // SAFETY: `tm` is plain data and localtime_r only writes into it.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            0
        } else {
            tm.tm_gmtoff as i64
        }
    }
}

#[cfg(not(unix))]
fn utc_offset(_secs: i64) -> i64 {
    0
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
        for d in [0, EPOCH_DAY, EPOCH_DAY + 51, EPOCH_DAY - 315, -800] {
            let (y, m, dd) = civil(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }
}
