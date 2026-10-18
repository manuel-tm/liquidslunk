//! Dates as the user types them, stored as epoch seconds at midnight UTC
//! (slunk's design §4.3 Dates). Release dates and date tags both use them.
//!
//! slunk worked them out with JavaScript's `Date.UTC()` and `toISOString()`.
//! These do the same with whole days, so they give the same epochs, refuse
//! the same dates, and print the same text.

use crate::text::trim;
use crate::{Error, Result};

const DAY: i64 = 86_400;

/// How much of a date is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePrecision {
    Year,
    Month,
    Day,
}

impl DatePrecision {
    /// "year", "month" or "day", as the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            DatePrecision::Year => "year",
            DatePrecision::Month => "month",
            DatePrecision::Day => "day",
        }
    }
}

/// A date, at midnight UTC on its first day, and how much of it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDate {
    pub epoch: i64,
    pub precision: DatePrecision,
}

/// Parses `2006`, `2006-01` or `2006-01-10`. Unknown parts become the first
/// one, at midnight UTC.
///
/// Years before 100 are refused, as slunk refused them: `Date.UTC()` reads
/// them as 1900 to 1999, so the date it made wasn't the one typed.
pub fn parse_date(text: &str) -> Result<ParsedDate> {
    let Some((year, month, day)) = date_parts(trim(text)) else {
        return Err(Error::Invalid(format!(
            "Not a date: {text}. Write it as 2006, 2006-01 or 2006-01-10."
        )));
    };
    let (m, d) = (month.unwrap_or(1), day.unwrap_or(1));
    if year < 100 || !(1..=12).contains(&m) || d < 1 || d > days_in_month(year, m) {
        return Err(Error::Invalid(format!("There's no such date: {text}")));
    }
    Ok(ParsedDate {
        epoch: days_from_civil(year, m, d) * DAY,
        precision: if day.is_some() {
            DatePrecision::Day
        } else if month.is_some() {
            DatePrecision::Month
        } else {
            DatePrecision::Year
        },
    })
}

/// The last day of a parsed date's year, month or day, at midnight UTC.
pub fn last_day(date: &ParsedDate) -> i64 {
    let (year, month, day) = civil_from_days(date.epoch.div_euclid(DAY));
    let next = match date.precision {
        DatePrecision::Year => days_from_civil(year + 1, month, day),
        DatePrecision::Month if month == 12 => days_from_civil(year + 1, 1, day),
        DatePrecision::Month => days_from_civil(year, month + 1, day),
        DatePrecision::Day => days_from_civil(year, month, day) + 1,
    };
    (next - 1) * DAY
}

/// A date with only its known parts: `2006`, `2006-01` or `2006-01-10`.
/// Years past 9999, or before 0, are written as `toISOString()` writes
/// them, with a sign and six digits.
pub fn format_date(epoch: i64, precision: DatePrecision) -> String {
    let (year, month, day) = civil_from_days(epoch.div_euclid(DAY));
    let year = if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else {
        format!("{}{:06}", if year < 0 { '-' } else { '+' }, year.abs())
    };
    let iso = format!("{year}-{month:02}-{day:02}");
    let length = match precision {
        DatePrecision::Year => year.len(),
        DatePrecision::Month => year.len() + 3,
        DatePrecision::Day => year.len() + 6,
    };
    iso[..length].to_string()
}

/// `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, in ASCII digits, as numbers.
fn date_parts(text: &str) -> Option<(i64, Option<i64>, Option<i64>)> {
    let number = |part: &str, digits: usize| {
        (part.len() == digits && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<i64>().ok())
            .flatten()
    };
    let mut parts = text.split('-');
    let year = number(parts.next()?, 4)?;
    let mut next = || match parts.next() {
        Some(part) => number(part, 2).map(Some),
        None => Some(None),
    };
    let month = next()?;
    let day = next()?;
    parts.next().is_none().then_some((year, month, day))
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar, as `Date`
/// counts them. A day past the month's end runs into the next month.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    // Howard Hinnant's algorithm, with years starting in March
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The year, month and day of a day since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = if march_month < 10 {
        march_month + 3
    } else {
        march_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ParsedDate {
        parse_date(text).unwrap()
    }

    fn error(text: &str) -> String {
        parse_date(text).unwrap_err().to_string()
    }

    #[test]
    fn dates_are_a_year_a_month_or_a_day_at_midnight_utc() {
        // What Date.UTC() gives in Node
        assert_eq!(
            parse("2006"),
            ParsedDate {
                epoch: 1_136_073_600,
                precision: DatePrecision::Year
            }
        );
        assert_eq!(
            parse(" 2006-01 "),
            ParsedDate {
                epoch: 1_136_073_600,
                precision: DatePrecision::Month
            }
        );
        assert_eq!(
            parse("2006-01-10"),
            ParsedDate {
                epoch: 1_136_851_200,
                precision: DatePrecision::Day
            }
        );
        assert_eq!(parse("1969-12-31").epoch, -86_400);
        assert_eq!(parse("2024-02-29").epoch, 1_709_164_800);
        assert_eq!(parse("0100-01-01").epoch, -59_011_459_200);
        assert_eq!(parse("9999-12-31").epoch, 253_402_214_400);
    }

    #[test]
    fn what_isnt_a_date_is_refused_with_what_was_typed() {
        assert_eq!(
            error(" 06 "),
            "Not a date:  06 . Write it as 2006, 2006-01 or 2006-01-10."
        );
        for text in [
            "2006-1",
            "2006-01-1",
            "2006/01",
            "2006-01-10-",
            "",
            "２００６",
        ] {
            assert!(error(text).starts_with("Not a date"), "{text}");
        }
        assert_eq!(error("2006-02-30"), "There's no such date: 2006-02-30");
        for text in [
            "2006-00",
            "2006-13",
            "2006-01-00",
            "2006-01-32",
            "2023-02-29",
        ] {
            assert!(error(text).starts_with("There's no such date"), "{text}");
        }
        // Date.UTC() reads years 0 to 99 as 1900 to 1999
        assert_eq!(error("0099"), "There's no such date: 0099");
        assert_eq!(error("0000-01-01"), "There's no such date: 0000-01-01");
    }

    #[test]
    fn a_dates_last_day_ends_its_year_month_or_day() {
        assert_eq!(
            format_date(last_day(&parse("2006")), DatePrecision::Day),
            "2006-12-31"
        );
        assert_eq!(
            format_date(last_day(&parse("2024-02")), DatePrecision::Day),
            "2024-02-29"
        );
        assert_eq!(
            format_date(last_day(&parse("2006-12")), DatePrecision::Day),
            "2006-12-31"
        );
        assert_eq!(last_day(&parse("2006-01-10")), parse("2006-01-10").epoch);
    }

    #[test]
    fn dates_print_only_their_known_parts() {
        assert_eq!(format_date(1_136_851_200, DatePrecision::Year), "2006");
        assert_eq!(format_date(1_136_851_200, DatePrecision::Month), "2006-01");
        assert_eq!(format_date(1_136_851_200, DatePrecision::Day), "2006-01-10");
        assert_eq!(format_date(-86_400, DatePrecision::Day), "1969-12-31");
        // As toISOString() writes years outside 0 to 9999
        assert_eq!(
            format_date(253_402_300_800, DatePrecision::Day),
            "+010000-01-01"
        );
        assert_eq!(
            format_date(-62_198_755_200, DatePrecision::Day),
            "-000001-01-01"
        );
        assert_eq!(
            format_date(-62_198_755_200, DatePrecision::Month),
            "-000001-01"
        );
    }

    #[test]
    fn days_round_trip_through_the_calendar() {
        for days in -800_000..800_000 {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
            assert!(day >= 1 && day <= days_in_month(year, month));
        }
    }
}
