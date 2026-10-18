//! Tag values as the user types them, and as the API shows them: text, a
//! number like `8.5`, or a date like `2006-01-10`.

use crate::dates::{DatePrecision, format_date, parse_date};
use crate::tagging::{Tag, TagType, TagValue};
use crate::text::{clean_text, trim};
use crate::{Error, Result};

/// A value as the user typed it, stored by its tag's type. The error names
/// the tag: "Score: Not a number: abc".
pub(crate) fn parse_value(tag: &Tag, text: &str) -> Result<TagValue> {
    let parsed = match tag.kind {
        TagType::Text => Ok(TagValue::Text(clean_text(text))),
        TagType::Number => parse_number(text).map(TagValue::Number),
        TagType::Date => parse_date(text).map(|date| TagValue::Date(date.epoch)),
    };
    parsed.map_err(|error| Error::Invalid(format!("{}: {error}", tag.name)))
}

/// A number like `8`, `-3.25`, `.5` or `1e3`, as JavaScript's `Number()`
/// reads it, and finite.
pub(crate) fn parse_number(text: &str) -> Result<f64> {
    let trimmed = trim(text);
    match trimmed.parse::<f64>() {
        Ok(number) if is_decimal(trimmed) && number.is_finite() => Ok(number),
        _ => Err(Error::Invalid(format!("Not a number: {text}"))),
    }
}

/// `^[-+]?(\d+(\.\d*)?|\.\d+)(e[-+]?\d+)?$`, ignoring case, as slunk checked
/// it.
fn is_decimal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while bytes.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i - start
    };
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let whole = digits(&mut i);
    let mut fraction = 0;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        fraction = digits(&mut i);
    }
    if whole == 0 && fraction == 0 {
        return false;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == bytes.len()
}

/// A value as the API shows it: text, `8.5`, or `2006-01-10`.
pub fn format_value(value: &TagValue) -> String {
    match value {
        TagValue::Text(text) => text.clone(),
        TagValue::Number(number) => format_number(*number),
        TagValue::Date(epoch) => format_date(*epoch, DatePrecision::Day),
    }
}

/// Values that are the same, ignoring ASCII case for text, as SQLite's
/// NOCASE compares, have the same key.
pub(crate) fn same_value_key(value: &TagValue) -> String {
    match value {
        TagValue::Text(text) => text.to_ascii_lowercase(),
        TagValue::Number(number) => format_number(*number),
        TagValue::Date(epoch) => epoch.to_string(),
    }
}

/// A number as JavaScript's `String()` writes it: the fewest digits that
/// read back as the same number, `8.5`, `1000`, `1e+21` or `1e-7`.
pub fn format_number(number: f64) -> String {
    if number.is_nan() {
        return "NaN".into();
    }
    if number == 0.0 {
        // -0 too
        return "0".into();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }
        .into();
    }
    // Rust's `{:e}` writes the fewest digits, `8.5e0` or `1e21`. Of two
    // candidates as close, JavaScript takes the even one, which rounding the
    // exact value to that many digits gives.
    let shortest = format!("{:e}", number.abs());
    let precision = shortest
        .split_once('e')
        .map_or(0, |(mantissa, _)| mantissa.len().saturating_sub(2));
    let even = format!("{:.*e}", precision, number.abs());
    let scientific = if even.parse() == Ok(number.abs()) {
        even
    } else {
        shortest
    };
    let (mantissa, exponent) = scientific.split_once('e').expect("an exponent");
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    let exponent: i32 = exponent.parse().expect("an exponent");
    // ECMAScript's Number::toString: the digits are d1…dk × 10^(n − k)
    let k = digits.len() as i32;
    let n = exponent + 1;
    let text = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat(-n as usize))
    } else {
        let sign = if n - 1 < 0 { '-' } else { '+' };
        let rest = if k == 1 {
            String::new()
        } else {
            format!(".{}", &digits[1..])
        };
        format!("{}{rest}e{sign}{}", &digits[..1], (n - 1).abs())
    };
    if number < 0.0 {
        format!("-{text}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_decimals_and_finite() {
        for (text, number) in [
            ("8", 8.0),
            (" -3.25 ", -3.25),
            ("+5", 5.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("1e3", 1000.0),
            ("1E-3", 0.001),
            ("-0", -0.0),
            ("1e-400", 0.0),
        ] {
            assert_eq!(parse_number(text).unwrap(), number, "{text}");
        }
        for text in [
            "", ".", "e3", "1e", "0x10", "Infinity", "NaN", "1_000", "1,5", "1e400", "١",
        ] {
            assert_eq!(
                parse_number(text).unwrap_err().to_string(),
                format!("Not a number: {text}")
            );
        }
    }

    #[test]
    fn numbers_print_as_javascript_prints_them() {
        // What String() gives in Node
        for (number, text) in [
            (8.0, "8"),
            (8.5, "8.5"),
            (-3.25, "-3.25"),
            (-0.0, "0"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1000.0, "1000"),
            (1e21, "1e+21"),
            (123456789012345680000.0, "123456789012345680000"),
            (1.5e300, "1.5e+300"),
            (0.000001, "0.000001"),
            (1e-7, "1e-7"),
            (-1.25e-7, "-1.25e-7"),
            (123.456, "123.456"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (2f64.powi(53), "9007199254740992"),
            // Halfway between …005.2 and …005.3, so the even one
            (f64::from_bits(0xc308_1ecd_2565_992a), "-848658235700005.2"),
        ] {
            assert_eq!(format_number(number), text, "{number:e}");
        }
    }
}
