use crate::{
    Result,
    calendar::{Calendar, month_length},
};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub struct Logger {
    file: Option<File>,
}

fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character < ' ' => {
                output.push_str(&format!("\\u{:04x}", u32::from(character)))
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn timestamp() -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Read current time: {error}"))?;
    let mut days = now.as_secs() / 86400;
    let mut year = 1970;
    loop {
        let length = if month_length(year, 2) == Some(29) {
            366
        } else {
            365
        };
        if days < length {
            break;
        }
        days -= length;
        year += 1;
    }
    let mut month = 1;
    loop {
        let length = u64::from(month_length(year, month).ok_or("Invalid log timestamp month")?);
        if days < length {
            break;
        }
        days -= length;
        month += 1;
    }
    let calendar = Calendar {
        year,
        month,
        day: days as u32 + 1,
    };
    let seconds = now.as_secs() % 86400;
    Ok(format!(
        "{}T{:02}:{:02}:{:02}.{:03}Z",
        calendar.text(),
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        now.subsec_millis()
    ))
}

impl Logger {
    pub fn start(&mut self, name: &str) -> Result<()> {
        let (stamp, errors) = crate::date::execute(&["+%Y%m%d_%H%M%S".into()], None)?;
        if !errors.is_empty() {
            crate::write_stderr(&errors)?;
        }
        let stamp = stamp.trim();
        if !crate::normalize::matches(r"^\d{8}_\d{6}$", stamp)? {
            return Err("Unexpected debug timestamp from date".into());
        }
        let safe: String = name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        let path = std::env::current_dir()
            .map_err(|error| crate::io_error("Read current directory", error))?
            .join(format!("_{stamp}_{safe}.log"));
        self.file = Some(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map_err(|error| crate::io_error("Create debug log", error))?,
        );
        self.record("Environment and invocation", &[("arguments", format!("{:?}", std::env::args_os().skip(1).collect::<Vec<_>>())),
            ("cwd", std::env::current_dir().map_err(|error| crate::io_error("Read current directory", error))?.display().to_string()),
            ("executable", name.into()), ("runtime", "native Rust".into()),
            ("TZ", std::env::var("TZ").unwrap_or_default()), ("LANG", std::env::var("LANG").unwrap_or_default()),
            ("sensitivity", "Contains supplied inputs and paths. No credentials or network traffic are collected.".into())])?;
        crate::write_stderr(&format!("{name}: debug log: {}\n", path.display()))
    }

    pub fn record(&mut self, step: &str, details: &[(&str, String)]) -> Result<()> {
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        let mut record = format!("{} {step}\n{{\n", timestamp()?);
        for (index, (key, value)) in details.iter().enumerate() {
            record.push_str(&format!(
                "  {}: {}{}\n",
                quote(key),
                quote(value),
                if index + 1 == details.len() { "" } else { "," }
            ));
        }
        record.push_str("}\n");
        file.write_all(record.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|error| crate::io_error("Write debug log", error))
    }

    pub fn finish(&mut self) -> Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()
                .and_then(|()| file.sync_all())
                .map_err(|error| crate::io_error("Flush debug log", error))?;
        }
        Ok(())
    }
}
