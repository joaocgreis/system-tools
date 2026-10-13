use crate::Result;

pub const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

pub fn month_length(year: i32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            29
        } else {
            28
        }),
        _ => None,
    }
}

impl Calendar {
    pub fn validate(self) -> Result<()> {
        let length = month_length(self.year, self.month)
            .ok_or_else(|| format!("Invalid calendar month: {}", self.month))?;
        if self.day == 0 || self.day > length {
            return Err(format!("Invalid calendar date: {}", self.text()));
        }
        Ok(())
    }

    pub fn text(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn next_day(self) -> Result<Self> {
        self.validate()?;
        if self.day < month_length(self.year, self.month).ok_or("Invalid calendar month")? {
            return Ok(Self {
                day: self.day + 1,
                ..self
            });
        }
        if self.month < 12 {
            return Ok(Self {
                month: self.month + 1,
                day: 1,
                ..self
            });
        }
        Ok(Self {
            year: self.year.checked_add(1).ok_or("Calendar year overflow")?,
            month: 1,
            day: 1,
        })
    }
}

pub fn encode(calendar: Calendar) -> Result<String> {
    calendar.validate()?;
    if !(2000..=2061).contains(&calendar.year) {
        return Err(format!(
            "Date {} cannot be represented; supported range is 2000-01-01 through 2061-12-31",
            calendar.text()
        ));
    }
    Ok([
        ALPHABET[(calendar.year - 2000) as usize],
        ALPHABET[calendar.month as usize],
        ALPHABET[calendar.day as usize],
    ]
    .into_iter()
    .map(char::from)
    .collect())
}

pub fn decode(code: &str) -> Result<Calendar> {
    if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(
            "A compact date must contain exactly three characters from 0-9, A-Z, a-z".into(),
        );
    }
    let mut fields = [0_u32; 3];
    for (index, byte) in code.bytes().enumerate() {
        fields[index] = ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or("Invalid compact date alphabet")? as u32;
    }
    let calendar = Calendar {
        year: 2000 + fields[0] as i32,
        month: fields[1],
        day: fields[2],
    };
    if !(1..=12).contains(&calendar.month) {
        return Err("Invalid month character; use 1-9 or A-C".into());
    }
    calendar
        .validate()
        .map_err(|error| format!("Invalid day character or calendar date: {error}"))?;
    Ok(calendar)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_representable_day_round_trips() {
        let mut count = 0;
        for year in 2000..=2061 {
            for month in 1..=12 {
                for day in 1..=month_length(year, month).unwrap() {
                    let calendar = Calendar { year, month, day };
                    assert_eq!(decode(&encode(calendar).unwrap()).unwrap(), calendar);
                    count += 1;
                }
            }
        }
        assert_eq!(count, 22_646);
        for year in [1999, 2062] {
            assert!(
                encode(Calendar {
                    year,
                    month: 1,
                    day: 1
                })
                .is_err()
            );
        }
    }

    #[test]
    fn all_codes_have_exactly_the_intended_domain() {
        let mut count = 0;
        for (year, y) in ALPHABET.iter().enumerate() {
            for (month, m) in ALPHABET.iter().enumerate() {
                for (day, d) in ALPHABET.iter().enumerate() {
                    let code: String = [*y, *m, *d].into_iter().map(char::from).collect();
                    let calendar = Calendar {
                        year: 2000 + year as i32,
                        month: month as u32,
                        day: day as u32,
                    };
                    let valid = calendar.validate().is_ok();
                    assert_eq!(decode(&code).is_ok(), valid, "{code}");
                    if valid {
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(count, 22_646);
    }
}
