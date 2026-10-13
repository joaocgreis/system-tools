mod calendar;
mod date;
mod debug;
mod normalize;
mod options;

use debug::Logger;
use std::{
    io::{self, Write},
    os::unix::fs::MetadataExt,
    path::Path,
    process::ExitCode,
};
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Encode,
    Decode,
}

pub fn io_error(action: &str, error: io::Error) -> String {
    let code = match error.kind() {
        io::ErrorKind::NotFound => "ENOENT",
        io::ErrorKind::AlreadyExists => "EEXIST",
        io::ErrorKind::PermissionDenied => "EACCES",
        _ => "I/O error",
    };
    format!("{action}: {code}: {error}")
}

fn write_stdout(text: &str) -> Result<()> {
    let mut output = io::stdout().lock();
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .map_err(|error| io_error("Write stdout", error))
}
pub fn write_stderr(text: &str) -> Result<()> {
    let mut output = io::stderr().lock();
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .map_err(|error| io_error("Write stderr", error))
}

fn run(tool: Tool, name: &str, logger: &mut Logger) -> Result<()> {
    let options = options::parse(tool)?;
    let command = if tool == Tool::Encode {
        "cdate"
    } else {
        "ddate"
    };
    if options.help {
        return write_stdout(if tool == Tool::Encode {
            include_str!("../../docs/cdate-help.txt")
        } else {
            include_str!("../../docs/ddate-help.txt")
        });
    }
    if options.version {
        return write_stdout(&format!("{command} {}\n", env!("CARGO_PKG_VERSION")));
    }
    if options.debug {
        logger.start(name)?;
    }
    logger.record("Execution plan", &[("options", format!("{options:?}")),
        ("reason", "Validate input, resolve its instant, independently format the output, then flush one result.".into())])?;
    if tool == Tool::Encode {
        let mut value = if let Some(reference) = &options.reference {
            logger.record(
                "Read reference metadata",
                &[
                    ("path", reference.clone()),
                    (
                        "reason",
                        "Use the modification instant without reading file contents.".into(),
                    ),
                ],
            )?;
            let metadata = std::fs::metadata(reference)
                .map_err(|error| io_error("Read reference metadata", error))?;
            Some(date::epoch_from_nanoseconds(
                i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec()),
            ))
        } else {
            options
                .input
                .as_deref()
                .map(|input| normalize::input(input, &options.input_format))
                .transpose()?
        };
        let mut input_zone = options.input_zone.as_deref();
        if options.input_format == "javascript" {
            let raw = options
                .input
                .as_deref()
                .ok_or("--input-format requires a date input")?;
            normalize::validate_javascript(raw)?;
            value = value
                .map(|value| normalize::javascript_year(value, raw))
                .transpose()?;
            if normalize::javascript_date_only(raw)? {
                input_zone = Some("UTC0");
            }
        }
        let mut instant = date::resolve(value.as_deref(), input_zone, logger)?;
        if options.input_format == "javascript" {
            // Date.parse stores integer milliseconds; retain its precision rule.
            instant = instant.div_euclid(1_000_000) * 1_000_000;
        }
        value = Some(date::epoch_from_nanoseconds(instant));
        let selected = date::run(
            value.as_deref(),
            &["+%Y-%m-%d".into()],
            options.output_zone.as_deref(),
            logger,
        )?;
        let calendar = normalize::calendar_from_text(selected.trim())?;
        let result = calendar::encode(calendar)?;
        logger.record(
            "Validated range and encoding",
            &[
                ("calendar", calendar.text()),
                ("result", result.clone()),
                (
                    "reason",
                    "The day exists and all three fields fit the compact alphabet.".into(),
                ),
            ],
        )?;
        write_stdout(&format!("{result}\n"))?;
    } else {
        let (code, supplied_zone) = normalize::compact(
            options
                .input
                .as_deref()
                .ok_or("A compact date is required")?,
        )?;
        let calendar = calendar::decode(&code)?;
        let input_zone = if options.input_zone_explicit {
            options.input_zone.as_deref()
        } else {
            Some(supplied_zone.as_deref().unwrap_or("UTC0"))
        };
        let midnight = format!("{} 00:00:00", calendar.text());
        let instant = date::resolve(Some(&midnight), input_zone, logger)?;
        let output = date::run(
            Some(&date::epoch_from_nanoseconds(instant)),
            &options.format_args,
            options.output_zone.as_deref(),
            logger,
        )?;
        logger.record(
            "Formatting validated",
            &[
                ("output", output.clone()),
                (
                    "reason",
                    "Format the validated input midnight in the independent output zone.".into(),
                ),
            ],
        )?;
        write_stdout(&output)?;
    }
    logger.record(
        "Completed",
        &[(
            "next",
            "Flush and close the private log, then return success.".into(),
        )],
    )
}

pub fn entry(tool: Tool) -> ExitCode {
    let name = std::env::args_os()
        .next()
        .and_then(|path| {
            Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| {
            if tool == Tool::Encode {
                "cdate".into()
            } else {
                "ddate".into()
            }
        });
    let mut logger = Logger::default();
    let result = run(tool, &name, &mut logger);
    let mut success = result.is_ok();
    if let Err(error) = result {
        if let Err(logging_error) = logger.record("Failed", &[("error", error.clone())]) {
            // The primary failure and logging failure must both remain visible.
            if write_stderr(&format!(
                "{name}: could not record failure: {logging_error}\n"
            ))
            .is_err()
            {
                success = false;
            }
        }
        if write_stderr(&format!("{name}: {error}\n")).is_err() {
            success = false;
        }
    }
    if let Err(error) = logger.finish() {
        success = false;
        if write_stderr(&format!("{name}: could not close debug log: {error}\n")).is_err() {
            return ExitCode::FAILURE;
        }
    }
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
