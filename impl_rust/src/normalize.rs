use crate::{Result, calendar::Calendar, date::epoch_from_nanoseconds};
use regex_lite::{Captures, Regex};

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

pub fn regex(pattern: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|error| format!("Internal regular expression error: {error}"))
}
pub fn matches(pattern: &str, value: &str) -> Result<bool> {
    Ok(regex(pattern)?.is_match(value))
}
fn captures<'a>(pattern: &str, value: &'a str) -> Result<Option<Captures<'a>>> {
    Ok(regex(pattern)?.captures(value))
}
fn group<'a>(captures: &Captures<'a>, index: usize) -> &'a str {
    captures.get(index).map_or("", |value| value.as_str())
}
fn number(value: &str) -> Result<u32> {
    value
        .parse()
        .map_err(|_| format!("Invalid numeric calendar/clock field: {value}"))
}

pub fn epoch(input: &str, milliseconds: bool) -> Result<String> {
    let captures = captures(r"^([+-]?)(\d+)(?:\.(\d+))?$", input)?
        .ok_or_else(|| format!("Invalid Unix timestamp: {input}"))?;
    let precision = if milliseconds { 6 } else { 9 };
    let fraction = group(&captures, 3);
    if fraction.len() > precision {
        return Err(format!(
            "Unix timestamp supports at most {precision} fractional digits"
        ));
    }
    let integer: i128 = group(&captures, 2)
        .parse()
        .map_err(|_| "date could not parse the timestamp integer range")?;
    let fraction: i128 = format!("{fraction:0<precision$}")
        .parse()
        .map_err(|_| "Invalid timestamp fraction")?;
    let scale = if milliseconds {
        1_000_000
    } else {
        1_000_000_000
    };
    let mut nanoseconds = integer
        .checked_mul(scale)
        .and_then(|value| value.checked_add(fraction))
        .ok_or("date could not parse the timestamp integer range")?;
    if group(&captures, 1) == "-" {
        nanoseconds = -nanoseconds;
    }
    Ok(epoch_from_nanoseconds(nanoseconds))
}

fn clock(input: &str) -> Result<String> {
    let mut value = input.to_owned();
    if let Some(captures) = captures(
        r"(?i)^(\d{4}-\d{2}-\d{2})[Tt ](\d{1,2}):(\d{2})(?::(\d{2})([.,]\d{1,9})?)?\s*(AM|PM)(?:\s+(.*))?$",
        &value,
    )? {
        let hour = number(group(&captures, 2))?;
        if !(1..=12).contains(&hour) {
            return Err("Invalid 12-hour clock time".into());
        }
        let converted = hour % 12
            + if group(&captures, 6).eq_ignore_ascii_case("PM") {
                12
            } else {
                0
            };
        value = format!(
            "{}T{converted:02}:{}:{}{}{}",
            group(&captures, 1),
            group(&captures, 3),
            if group(&captures, 4).is_empty() {
                "00"
            } else {
                group(&captures, 4)
            },
            group(&captures, 5),
            if group(&captures, 7).is_empty() {
                String::new()
            } else {
                format!(" {}", group(&captures, 7))
            }
        );
    }
    let Some(captures) = captures(
        r"^(\d{4}-\d{2}-\d{2})[Tt ](\d{1,2})(?::(\d{2})(?::(\d{2})([.,]\d{1,9})?)?)?\s*([Zz]|[+-]\d{2}(?::?\d{2})?(?::?\d{2})?)?$",
        &value,
    )?
    else {
        return Ok(value);
    };
    let hour = number(group(&captures, 2))?;
    let minute = if group(&captures, 3).is_empty() {
        0
    } else {
        number(group(&captures, 3))?
    };
    let second = if group(&captures, 4).is_empty() {
        0
    } else {
        number(group(&captures, 4))?
    };
    let fraction = group(&captures, 5).replace(',', ".");
    if hour > 24
        || minute > 59
        || second > 59
        || (hour == 24
            && (minute != 0
                || second != 0
                || fraction.bytes().any(|byte| (b'1'..=b'9').contains(&byte))))
    {
        return Err("Invalid clock time; 24:00 is allowed only as the end of a day".into());
    }
    let offset = group(&captures, 6);
    if offset.starts_with(['+', '-']) {
        let mut digits = offset[1..].replace(':', "");
        while digits.len() < 6 {
            digits.push('0');
        }
        if number(&digits[..2])? > 23 || number(&digits[2..4])? > 59 || number(&digits[4..6])? > 59
        {
            return Err("Invalid numeric time-zone offset".into());
        }
    }
    let mut day = group(&captures, 1).to_owned();
    if hour == 24 {
        day = calendar_from_text(&day)?.next_day()?.text();
    }
    let offset = if offset.len() == 3 && offset.starts_with(['+', '-']) {
        format!("{offset}:00")
    } else {
        offset.to_ascii_uppercase()
    };
    Ok(format!(
        "{day}T{:02}:{minute:02}:{second:02}{fraction}{offset}",
        if hour == 24 { 0 } else { hour }
    ))
}

pub fn calendar_from_text(input: &str) -> Result<Calendar> {
    let captures = captures(r"^([+-]?\d{4,})-(\d{2})-(\d{2})$", input)?
        .ok_or_else(|| format!("Unexpected calendar output from date: {input}"))?;
    let calendar = Calendar {
        year: group(&captures, 1)
            .parse()
            .map_err(|_| "Invalid calendar year")?,
        month: number(group(&captures, 2))?,
        day: number(group(&captures, 3))?,
    };
    calendar.validate()?;
    Ok(calendar)
}

fn numeric(input: &str, order: &str) -> Result<Option<String>> {
    let fields: [&str; 3];
    let mut suffix: String;
    if let Some(captures) = captures(r"^(\d{8})((?:[Tt ,].*)?)$", input)? {
        let digits = group(&captures, 1);
        fields = if order == "mdy" || order == "dmy" {
            [&digits[..2], &digits[2..4], &digits[4..]]
        } else {
            [&digits[..4], &digits[4..6], &digits[6..]]
        };
        suffix = group(&captures, 2).into();
    } else if let Some(captures) = captures(
        r"^(\d{1,4})([-/.])(\d{1,2})([-/.])(\d{1,4})((?:[Tt ,].*)?)$",
        input,
    )? {
        if group(&captures, 2) != group(&captures, 4) {
            return Ok(None);
        }
        fields = [
            group(&captures, 1),
            group(&captures, 3),
            group(&captures, 5),
        ];
        suffix = group(&captures, 6).into();
    } else {
        if let Some(captures) = captures(r"^(\d{4})(?:-(\d{1,2}))?$", input)? {
            if order == "auto" || order == "ymd" {
                let calendar = Calendar {
                    year: number(group(&captures, 1))? as i32,
                    month: if group(&captures, 2).is_empty() {
                        1
                    } else {
                        number(group(&captures, 2))?
                    },
                    day: 1,
                };
                calendar.validate()?;
                return Ok(Some(calendar.text()));
            }
        }
        return Ok(None);
    }
    let [first, middle, last] = fields;
    let detected = if order == "auto" {
        if first.len() == 4 {
            "ymd"
        } else if number(first)? > 12 {
            "dmy"
        } else {
            "mdy"
        }
    } else {
        order
    };
    let year_text = if detected == "ymd" { first } else { last };
    if year_text.len() != 4 && !(detected != "ymd" && year_text.len() == 2) {
        return Err(
            "Numeric dates need a four-digit year, or a two-digit year in mdy/dmy form".into(),
        );
    }
    let mut year = number(year_text)? as i32;
    if year_text.len() == 2 {
        year += if year <= 68 { 2000 } else { 1900 };
    }
    let calendar = Calendar {
        year,
        month: number(if detected == "mdy" { first } else { middle })?,
        day: number(if detected == "ymd" {
            last
        } else if detected == "mdy" {
            middle
        } else {
            first
        })?,
    };
    calendar.validate()?;
    if let Some(rest) = suffix.strip_prefix(',') {
        suffix = format!(" {}", rest.trim_start());
    }
    Ok(Some(clock(&(calendar.text() + &suffix))?))
}

fn javascript_string(input: &str) -> Result<Option<String>> {
    let Some(captures) = captures(
        r"^(?:Sun|Mon|Tue|Wed|Thu|Fri|Sat) (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(\d{1,2}) (\d{4})(?: (\d{2}:\d{2}:\d{2}) GMT([+-]\d{4})(?: \([^\r\n]*\))?)?$",
        input,
    )?
    else {
        return Ok(None);
    };
    let month_name = group(&captures, 1).to_ascii_lowercase();
    let month = MONTHS
        .iter()
        .position(|month| *month == month_name)
        .ok_or("Invalid JavaScript month name")?
        + 1;
    let calendar = Calendar {
        year: number(group(&captures, 3))? as i32,
        month: month as u32,
        day: number(group(&captures, 2))?,
    };
    calendar.validate()?;
    Ok(Some(if group(&captures, 4).is_empty() {
        calendar.text()
    } else {
        clock(&format!(
            "{}T{}{}",
            calendar.text(),
            group(&captures, 4),
            group(&captures, 5)
        ))?
    }))
}

fn time_only(input: &str) -> Result<Option<String>> {
    if let Some(captures) = captures(
        r"^(\d{2}:\d{2}:\d{2}) GMT([+-]\d{4})(?: \([^\r\n]*\))?$",
        input,
    )? {
        return Ok(Some(format!(
            "{} {}",
            group(&captures, 1),
            group(&captures, 2)
        )));
    }
    let Some(captures) = captures(r"(?i)^(\d{1,2}):(\d{2})(?::(\d{2}))?\s*(AM|PM)$", input)? else {
        return Ok(None);
    };
    let hour = number(group(&captures, 1))?;
    let minute = number(group(&captures, 2))?;
    let second = if group(&captures, 3).is_empty() {
        0
    } else {
        number(group(&captures, 3))?
    };
    if !(1..=12).contains(&hour) || minute > 59 || second > 59 {
        return Err("Invalid 12-hour clock time".into());
    }
    let hour = hour % 12
        + if group(&captures, 4).eq_ignore_ascii_case("PM") {
            12
        } else {
            0
        };
    Ok(Some(format!("{hour:02}:{minute:02}:{second:02}")))
}

pub fn input(raw: &str, format: &str) -> Result<String> {
    let input: String = raw
        .trim()
        .chars()
        .filter(|character| !['\u{200e}', '\u{200f}', '\u{061c}'].contains(character))
        .map(|character| {
            if ['\u{00a0}', '\u{202f}'].contains(&character) {
                ' '
            } else {
                character
            }
        })
        .collect();
    if input.is_empty() || input.contains(['\0', '\r', '\n']) {
        return Err("Provide one nonempty date or timestamp without NUL or line breaks".into());
    }
    if format == "unix" || format == "unix-ms" {
        return epoch(
            input.strip_prefix('@').unwrap_or(&input),
            format == "unix-ms",
        );
    }
    if format == "date" {
        return Ok(input);
    }
    if format == "auto" {
        if let Some(value) = input.strip_prefix('@') {
            return epoch(value, false);
        }
        if matches(r"^[+-]?\d{10}(?:\.\d+)?$", &input)? {
            return epoch(&input, false);
        }
        if matches(r"^[+-]?\d{13}(?:\.\d+)?$", &input)? {
            return epoch(&input, true);
        }
    }
    let order = if ["mdy", "dmy", "ymd"].contains(&format) {
        format
    } else {
        "auto"
    };
    if let Some(value) = numeric(&input, order)? {
        return Ok(value);
    }
    if order != "auto" {
        return Err(format!(
            "Input does not match {order} numeric calendar format"
        ));
    }
    if let Some(value) = javascript_string(&input)? {
        return Ok(value);
    }
    if let Some(value) = time_only(&input)? {
        return Ok(value);
    }
    Ok(input)
}

pub fn absolute(value: &str) -> Result<bool> {
    let calendar = matches(r"^\d{1,4}[-/.]\d{1,2}[-/.]\d{1,4}(?:[T ]|$)", value)?
        || (matches(
            r"(?i)\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\b",
            value,
        )? && matches(r"(?:^|\s)\d{4}(?:\s|$)", value)?);
    let clock = matches(
        r"(?i)\d{1,2}:\d{2}(?::\d{2}(?:[.,]\d+)?)?\s*(?:Z|UTC|GMT|UT|[+-]\d{2}(?::?\d{2})?(?::?\d{2})?)(?:\s+\d{4})?$",
        value,
    )?;
    let standard = matches(
        r"^[A-Za-z]{3} [A-Za-z]{3} +\d{1,2} \d{2}:\d{2}:\d{2} (?:[A-Za-z]{1,6}|[+-]\d{2}(?::?\d{2})?) \d{4}$",
        value,
    )?;
    Ok(value.starts_with('@') || (calendar && clock) || standard)
}

pub fn javascript_date_only(raw: &str) -> Result<bool> {
    matches(r"^\d{4}(?:-\d{2}(?:-\d{2})?)?$", raw.trim())
}

pub fn validate_javascript(raw: &str) -> Result<()> {
    let input = raw.trim();
    if matches(r"^\d{4}-\d{2}-\d{2}T", input)? {
        if !matches(
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?$",
            input,
        )? {
            return Err("JavaScript Date.parse cannot parse this input".into());
        }
    } else if matches(r"^\d{8}$", input)?
        || input.starts_with('@')
        || !matches(r"\b\d{4}\b|^\d{1,2}[-/.]\d{1,2}[-/.]\d{2}$", input)?
    {
        return Err("JavaScript Date.parse cannot parse this input".into());
    }
    if let Some(captures) = captures(r"^(\d{1,2})[-/.]\d{1,2}[-/.]\d{2,4}", input)? {
        if number(group(&captures, 1))? > 12 {
            return Err("JavaScript Date.parse cannot parse this input".into());
        }
    }
    // Relative grammar belongs to date, not Date.parse. Standard date strings
    // may still contain weekday names and full English month names.
    if matches(
        r"(?i)\b(?:ago|next|last|tomorrow|yesterday|today|now|seconds?|minutes?|hours?|days?|weeks?|months?|years?)\b",
        input,
    )? {
        return Err("JavaScript Date.parse cannot parse this input".into());
    }
    Ok(())
}

pub fn javascript_year(value: String, raw: &str) -> Result<String> {
    // V8's legacy two-digit-year window is 00-49/50-99, rather than
    // date's 00-68/69-99. The numeric calendar has already been validated.
    if let Some(fields) = captures(
        r"^\d{1,2}[-/.]\d{1,2}[-/.](\d{2})(?:[Tt ,].*)?$",
        raw.trim(),
    )? {
        let year = number(group(&fields, 1))?;
        if (50..=68).contains(&year) {
            return Ok(format!("{}{}", 1900 + year, &value[4..]));
        }
    }
    Ok(value)
}

pub fn compact(input: &str) -> Result<(String, Option<String>)> {
    let captures = captures(r"^([0-9A-Za-z]{3})(?: +([^\s]+)|(Z|[+-]\d{2}(?::?\d{2})?))?$", input)?
        .ok_or("A compact date must contain exactly three alphabet characters, optionally followed by a time zone")?;
    let code = group(&captures, 1).to_owned();
    let zone = if !group(&captures, 2).is_empty() {
        group(&captures, 2)
    } else {
        group(&captures, 3)
    };
    if zone.is_empty() {
        return Ok((code, None));
    }
    if ["Z", "UTC", "GMT"].contains(&zone) {
        return Ok((code, Some("UTC0".into())));
    }
    if let Some(offset) = self::captures(r"^([+-])(\d{2})(?::?(\d{2}))?$", zone)? {
        let hours = number(group(&offset, 2))?;
        let minutes = if group(&offset, 3).is_empty() {
            0
        } else {
            number(group(&offset, 3))?
        };
        if hours > 23 || minutes > 59 {
            return Err(format!("Invalid numeric input time-zone offset: {zone}"));
        }
        return Ok((
            code,
            Some(format!(
                "UTC{}{hours:02}:{minutes:02}",
                if group(&offset, 1) == "+" { "-" } else { "+" }
            )),
        ));
    }
    Ok((code, Some(crate::options::validate_zone(zone)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_epoch_units_and_negative_fractions() {
        assert_eq!(epoch("-0.000000001", false).unwrap(), "@-0.000000001");
        assert_eq!(
            epoch("1263167999999.999999", true).unwrap(),
            "@1263167999.999999999"
        );
        assert!(epoch("1.0000000001", false).is_err());
        assert!(epoch("1e12", true).is_err());
    }
}
