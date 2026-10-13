use crate::{Result, Tool, normalize::matches};
use lexopt::Arg;
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf};

#[derive(Debug)]
pub struct Options {
    pub help: bool,
    pub version: bool,
    pub debug: bool,
    pub input: Option<String>,
    pub input_format: String,
    pub reference: Option<String>,
    pub input_zone: Option<String>,
    pub input_zone_explicit: bool,
    pub output_zone: Option<String>,
    pub format_args: Vec<String>,
}

fn zone_file(root: &std::path::Path, name: &str) -> Result<Option<PathBuf>> {
    let mut path = root.to_path_buf();
    for component in name.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Ok(None);
        }
        let direct = path.join(component);
        if direct.exists() {
            path = direct;
        } else {
            if !path.is_dir() {
                return Ok(None);
            }
            let mut found = None;
            for entry in std::fs::read_dir(&path)
                .map_err(|error| crate::io_error("Read time-zone directory", error))?
            {
                let entry =
                    entry.map_err(|error| crate::io_error("Read time-zone entry", error))?;
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(component)
                {
                    found = Some(entry.path());
                    break;
                }
            }
            let Some(entry) = found else {
                return Ok(None);
            };
            path = entry;
        }
    }
    if !path.is_file() {
        return Ok(None);
    }
    let mut magic = [0_u8; 4];
    File::open(&path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map_err(|error| crate::io_error("Read time-zone data", error))?;
    Ok((magic == *b"TZif").then_some(path))
}

pub fn validate_zone(name: &str) -> Result<String> {
    if name.is_empty()
        || name.starts_with('/')
        || name.contains('\0')
        || ["localtime", "Factory"]
            .iter()
            .any(|invalid| name.eq_ignore_ascii_case(invalid))
        || name.to_ascii_lowercase().starts_with("posix/")
        || name.to_ascii_lowercase().starts_with("right/")
    {
        return Err(format!("Invalid IANA time zone: {name}"));
    }
    let root = std::env::var_os("TZDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/zoneinfo"));
    let mut candidate = name.to_owned();
    let aliases = root.join("tzdata.zi");
    let mut path = zone_file(&root, &candidate)?;
    if path.is_none() {
        // Some minimal systems ship zone files without compatibility symlinks.
        // Resolve their IANA Link records before declaring an alias invalid.
        let data = match std::fs::read_to_string(&aliases) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(crate::io_error("Read time-zone aliases", error)),
        };
        for _ in 0..8 {
            let target = data.lines().find_map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                (fields.len() == 3
                    && ["L", "Link"].contains(&fields[0])
                    && fields[2].eq_ignore_ascii_case(&candidate))
                .then(|| fields[1].to_owned())
            });
            let Some(target) = target else {
                break;
            };
            candidate = target;
            path = zone_file(&root, &candidate)?;
            if path.is_some() {
                break;
            }
        }
    }
    let path = path.ok_or_else(|| format!("Invalid IANA time zone: {name}"))?;
    let canonical = path
        .strip_prefix(&root)
        .map_err(|error| format!("Invalid IANA time zone: {error}"))?;
    canonical
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("Invalid IANA time zone: {name}"))
}

fn name(argument: Arg<'_>, tool: Tool) -> Result<&'static str> {
    let result = match argument {
        Arg::Short('h') | Arg::Long("help") => "help",
        Arg::Short('V') | Arg::Long("version") => "version",
        Arg::Short('d') | Arg::Long("debug") => "debug",
        Arg::Long("date") => "date",
        Arg::Short('u') | Arg::Long("utc" | "universal") => "utc",
        Arg::Short('l') | Arg::Long("local") => "local",
        Arg::Short('z') | Arg::Long("timezone") => "timezone",
        Arg::Long("input-utc") => "input-utc",
        Arg::Long("input-local") => "input-local",
        Arg::Long("input-timezone") => "input-timezone",
        Arg::Short('i') | Arg::Long("input-format") if tool == Tool::Encode => "input-format",
        Arg::Short('r') | Arg::Long("reference") if tool == Tool::Encode => "reference",
        Arg::Short('I') | Arg::Long("iso-8601") if tool == Tool::Decode => "iso-8601",
        Arg::Short('R') | Arg::Long("rfc-email" | "rfc-2822") if tool == Tool::Decode => {
            "rfc-email"
        }
        Arg::Long("rfc-3339") if tool == Tool::Decode => "rfc-3339",
        _ => return Err(format!("Unknown option: {argument:?}")),
    };
    Ok(result)
}

pub fn parse(tool: Tool) -> Result<Options> {
    let mut parser = lexopt::Parser::from_env();
    parser.set_short_equals(false);
    let mut values = BTreeMap::new();
    let mut positionals = Vec::new();
    while let Some(argument) = parser
        .next()
        .map_err(|error| format!("Invalid argument: {error}"))?
    {
        if let Arg::Value(value) = argument {
            positionals.push(
                value
                    .into_string()
                    .map_err(|_| "Invalid argument encoding")?,
            );
            continue;
        }
        let option = name(argument, tool)?;
        if values.contains_key(option) {
            return Err(format!("Option --{option} was specified more than once"));
        }
        let value = match option {
            "date" | "timezone" | "input-timezone" | "input-format" | "reference" | "rfc-3339" => {
                let attached = parser.optional_value();
                let is_attached = attached.is_some();
                let value = attached
                    .map(Ok)
                    .unwrap_or_else(|| parser.value())
                    .map_err(|error| format!("Invalid argument: {error}"))?
                    .into_string()
                    .map_err(|_| "Invalid argument encoding")?;
                if !is_attached && value.starts_with('-') && value.len() > 1 {
                    return Err(format!(
                        "Invalid argument for --{option}: use --{option}={value} for a value beginning with a hyphen"
                    ));
                }
                value
            }
            "iso-8601" => parser
                .optional_value()
                .map(|value| value.into_string().map_err(|_| "Invalid argument encoding"))
                .transpose()?
                .unwrap_or_else(|| "date".into()),
            _ => String::new(),
        };
        values.insert(option, value);
    }
    let has = |key| values.contains_key(key);
    let mut options = Options {
        help: has("help"),
        version: has("version"),
        debug: has("debug"),
        input: None,
        input_format: "auto".into(),
        reference: None,
        input_zone: Some("UTC0".into()),
        input_zone_explicit: false,
        output_zone: Some("UTC0".into()),
        format_args: Vec::new(),
    };
    if options.help || options.version {
        return Ok(options);
    }
    for group in [
        ["utc", "local", "timezone"],
        ["input-utc", "input-local", "input-timezone"],
    ] {
        if group.iter().filter(|key| has(**key)).count() > 1 {
            return Err(format!(
                "Choose only one of --{}, --{}, or --{}",
                group[0], group[1], group[2]
            ));
        }
    }
    if has("local") {
        options.output_zone = None;
    }
    if let Some(zone) = values.get("timezone") {
        options.output_zone = Some(validate_zone(zone)?);
    }
    if has("input-local") {
        options.input_zone = None;
    }
    if let Some(zone) = values.get("input-timezone") {
        options.input_zone = Some(validate_zone(zone)?);
    }
    options.input_zone_explicit = ["input-utc", "input-local", "input-timezone"]
        .iter()
        .any(|key| has(*key));
    if tool == Tool::Encode {
        if has("date") && !positionals.is_empty() {
            return Err("Use either --date or positional date words".into());
        }
        options.input = values
            .get("date")
            .cloned()
            .or_else(|| (!positionals.is_empty()).then(|| positionals.join(" ")));
        options.reference = values.get("reference").cloned();
        if options.reference.is_some() && options.input.is_some() {
            return Err("Use either --reference or a date input".into());
        }
        if options.reference.as_deref() == Some("") {
            return Err("--reference requires a nonempty file path".into());
        }
        options.input_format = values
            .get("input-format")
            .cloned()
            .unwrap_or_else(|| "auto".into());
        if ![
            "auto",
            "ymd",
            "mdy",
            "dmy",
            "unix",
            "unix-ms",
            "date",
            "javascript",
        ]
        .contains(&options.input_format.as_str())
        {
            return Err(format!("Unknown input format {}", options.input_format));
        }
        if options.input_format != "auto" && options.input.is_none() {
            return Err("--input-format requires a date input".into());
        }
    } else {
        let (mut formats, mut codes): (Vec<_>, Vec<_>) = positionals
            .into_iter()
            .partition(|value| value.starts_with('+'));
        if !has("date") && codes.len() == 1 && matches(r"^[0-9A-Za-z]{3}$", &codes[0])? {
            let offset = crate::normalize::regex(r"^\+\d{2}(?::?\d{2})?$")?;
            if let Some(index) = formats.iter().position(|value| offset.is_match(value)) {
                codes.push(formats.remove(index));
            }
        }
        if formats.len() > 1 {
            return Err("Supply at most one +FORMAT".into());
        }
        if codes.len() > if has("date") { 0 } else { 2 } {
            return Err("Supply exactly one compact date with an optional zone".into());
        }
        options.input = values
            .get("date")
            .cloned()
            .or_else(|| (!codes.is_empty()).then(|| codes.join(" ")));
        if options.input.is_none() {
            return Err("A compact date is required; use --help for usage".into());
        }
        let format_count = formats.len()
            + usize::from(has("iso-8601"))
            + usize::from(has("rfc-3339"))
            + usize::from(has("rfc-email"));
        if format_count > 1 {
            return Err("Choose only one +FORMAT, --iso-8601, --rfc-email, or --rfc-3339".into());
        }
        options.format_args = formats;
        for (option, allowed) in [
            (
                "iso-8601",
                &["date", "hours", "minutes", "seconds", "ns"][..],
            ),
            ("rfc-3339", &["date", "seconds", "ns"][..]),
        ] {
            if let Some(value) = values.get(option) {
                if !allowed.contains(&value.as_str()) {
                    return Err(format!(
                        "Invalid {} precision",
                        if option == "iso-8601" {
                            "ISO"
                        } else {
                            "RFC 3339"
                        }
                    ));
                }
                options.format_args.push(format!("--{option}={value}"));
            }
        }
        if has("rfc-email") {
            options.format_args.push("--rfc-email".into());
        }
        if !options.input_zone_explicit {
            options.input_zone = None; // The decoder fills this from the input or UTC.
        }
    }
    Ok(options)
}
