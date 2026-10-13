use crate::{Result, debug::Logger};
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const LIMIT: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);

fn read_output(stream: impl Read) -> Result<String> {
    let mut bytes = Vec::new();
    stream
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| crate::io_error("Read date output", error))?;
    if bytes.len() as u64 > LIMIT {
        return Err("date output exceeds the one-MiB limit".into());
    }
    String::from_utf8(bytes).map_err(|error| format!("Invalid UTF-8 from date: {error}"))
}

pub fn execute(args: &[String], zone: Option<&str>) -> Result<(String, String)> {
    let mut command = Command::new("date");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(zone) = zone {
        command.env("TZ", zone);
    }
    let mut child = command
        .spawn()
        .map_err(|error| crate::io_error("Start date", error))?;
    let stdout = child.stdout.take().ok_or("Missing date stdout pipe")?;
    let stderr = child.stderr.take().ok_or("Missing date stderr pipe")?;
    // Drain both pipes concurrently; waiting first can deadlock on a full pipe.
    let output_reader = thread::spawn(move || read_output(stdout));
    let error_reader = thread::spawn(move || read_output(stderr));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(1)),
            waiting => {
                let cause = match waiting {
                    Err(error) => crate::io_error("Wait for date", error),
                    _ => "date subprocess timed out after five seconds".into(),
                };
                let killed = child
                    .kill()
                    .map_err(|error| crate::io_error("Stop date", error));
                let reaped = child
                    .wait()
                    .map_err(|error| crate::io_error("Reap date", error));
                break match (killed, reaped) {
                    (Ok(()), Ok(_)) => Err(cause),
                    (Err(error), _) | (_, Err(error)) => Err(format!("{cause}; {error}")),
                };
            }
        }
    };
    let output = output_reader
        .join()
        .map_err(|_| "date stdout reader panicked")?;
    let errors = error_reader
        .join()
        .map_err(|_| "date stderr reader panicked")?;
    let status = status?;
    let output = output?;
    let errors = errors?;
    if !status.success() {
        return Err(format!(
            "date could not parse, interpret or format the input: {}",
            if errors.trim().is_empty() {
                status.to_string()
            } else {
                errors.trim().to_owned()
            }
        ));
    }
    Ok((output, errors))
}

pub fn run(
    value: Option<&str>,
    formats: &[String],
    zone: Option<&str>,
    logger: &mut Logger,
) -> Result<String> {
    let mut args = Vec::with_capacity(formats.len() + 1);
    if let Some(value) = value {
        args.push(format!("--date={value}"));
    }
    args.extend_from_slice(formats);
    logger.record(
        "Run date",
        &[
            ("arguments", format!("{args:?}")),
            ("zone", zone.unwrap_or("inherited system/TZ").into()),
            (
                "reason",
                "Resolve input or format an absolute instant without a shell.".into(),
            ),
        ],
    )?;
    let (output, errors) = execute(&args, zone)?;
    logger.record(
        "Date result",
        &[("stdout", output.clone()), ("stderr", errors.clone())],
    )?;
    if !errors.is_empty() {
        crate::write_stderr(&errors)?;
    }
    Ok(output)
}

pub fn epoch_from_nanoseconds(nanoseconds: i128) -> String {
    let sign = if nanoseconds < 0 { "-" } else { "" };
    let magnitude = nanoseconds.unsigned_abs();
    format!(
        "@{sign}{}.{:09}",
        magnitude / 1_000_000_000,
        magnitude % 1_000_000_000
    )
}

pub fn split_instant(output: &str) -> Result<(i128, &str)> {
    let mut lines = output.trim().split('\n');
    let epoch = lines.next().ok_or("Missing date epoch output")?;
    let clock = lines.next().ok_or("Missing date clock output")?;
    if lines.next().is_some() {
        return Err("Unexpected extra instant output from date".into());
    }
    let (seconds, fraction) = epoch
        .split_once('.')
        .ok_or("Unexpected instant output from date")?;
    if fraction.len() != 9 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Unexpected nanosecond output from date".into());
    }
    let seconds: i128 = seconds
        .parse()
        .map_err(|_| "Unexpected epoch seconds from date")?;
    let fraction: i128 = fraction
        .parse()
        .map_err(|_| "Unexpected epoch fraction from date")?;
    // date's %s is floor seconds. Add the nonnegative %N even before the epoch.
    let nanoseconds = seconds
        .checked_mul(1_000_000_000)
        .and_then(|seconds| seconds.checked_add(fraction))
        .ok_or("date epoch exceeds the supported integer range")?;
    Ok((nanoseconds, clock))
}

pub fn resolve(value: Option<&str>, zone: Option<&str>, logger: &mut Logger) -> Result<i128> {
    let absolute = value
        .map(crate::normalize::absolute)
        .transpose()?
        .unwrap_or(false);
    let parser_zone = if absolute { Some("UTC0") } else { zone };
    let output = run(
        value,
        &["+%s.%N\n%Y-%m-%d %H:%M:%S".into()],
        parser_zone,
        logger,
    )?;
    let (nanoseconds, clock) = split_instant(&output)?;
    if let Some(value) = value {
        let pattern = crate::normalize::regex(
            r"^(\d{4}-\d{2}-\d{2})(?:[T ](\d{2}:\d{2}:\d{2})(?:\.\d+)?)?$",
        )?;
        if let Some(captures) = pattern.captures(value) {
            let expected = format!(
                "{} {}",
                &captures[1],
                captures.get(2).map_or("00:00:00", |value| value.as_str())
            );
            if clock != expected {
                return Err(format!(
                    "The calendar/clock time does not exist in the input zone: {value}; supply an explicit offset or use --input-utc"
                ));
            }
        }
    }
    logger.record(
        "Input zone and clock validated",
        &[
            ("clock", clock.into()),
            ("instant", epoch_from_nanoseconds(nanoseconds)),
            ("zone", parser_zone.unwrap_or("inherited system/TZ").into()),
            (
                "reason",
                "Validate input before selecting the calendar day in the independent output zone."
                    .into(),
            ),
        ],
    )?;
    Ok(nanoseconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floor_seconds_and_exact_negative_fractions() {
        let (instant, _) = split_instant("-1.500000000\n1969-12-31 23:59:59\n").unwrap();
        assert_eq!(instant, -500_000_000);
        assert_eq!(epoch_from_nanoseconds(instant), "@-0.500000000");
        assert!(split_instant("1.1\nclock").is_err());
        assert!(split_instant("1.000000000\nclock\nextra").is_err());
    }
}
