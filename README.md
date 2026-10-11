# cdate and ddate

`cdate` turns a calendar date into a three-character code. `ddate` decodes that code and formats the date with the installed `date` command. The encoding preserves the original `cdate` alphabet and covers every Gregorian calendar day from **2000-01-01 through 2061-12-31**, inclusive.

```sh
./cdate 2010-01-11                      # A1B
./cdate 20100111                        # A1B
./cdate January 11, 2010                # A1B
./cdate -u 2010-01-11T13:41:54Z          # A1B
./ddate A1B +%F                         # 2010-01-11
./ddate -u A1B '+%A, %B %d, %Y'          # Monday, January 11, 2010
```

Run `./cdate` without input to encode today. Both commands use local time by default, following `date` and the `TZ` environment variable. The old script always used UTC; use `./cdate -u` for its time-zone behavior. An explicitly zoned timestamp can fall on a different calendar day after conversion to local time.

## Requirements and installation

Each command is an independent, extensionless executable containing TypeScript. It requires:

- Node.js 22.18 or newer on `PATH`.
- A GNU-compatible `date` on `PATH`, supporting `--date`, percent format directives, and the output options you use. The implementation was tested with uutils coreutils 0.8.0.
- A Unix environment with `/bin/sh` and time-zone data for the requested zones.

There is no package manifest, build step, third-party runtime dependency, or runtime transpiler. `npx` is needed only for development typechecks. BSD/macOS `date` is not compatible with `--date`; install GNU coreutils and make its `date` available on `PATH` before running these commands.

Run the files directly from this directory:

```sh
./cdate --help
./ddate --help
```

To install them on your own `PATH`, first preserve an existing command if you need it, then copy both executables:

```sh
mkdir -p "$HOME/.local/bin"
# If cdate is already installed, preserve it before the next command.
install -m 755 cdate ddate "$HOME/.local/bin/"
```

The install command replaces files with those names. Creating the commands in this repository does not install them or modify an existing `cdate` elsewhere.

## The encoding

All three positions use this alphabet, indexed from zero:

```text
0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz
```

The first character is the index `year - 2000`. The second is the month number, from 1 through 12. The third is the day number, from 1 through 31. This is a separate field encoding, rather than a single base-62 integer.

| Field | Character mapping |
| --- | --- |
| Year | `0`–`9` = 2000–2009; `A`–`Z` = 2010–2035; `a`–`z` = 2036–2061 |
| Month | `1`–`9` = January–September; `A`, `B`, `C` = October, November, December |
| Day | `1`–`9` = days 1–9; `A`–`V` = days 10–31 |

For `A1B`, `A` means 2010, `1` means January, and `B` means day 11. The earliest code is `011`; the latest is `zCV`. There are 22,646 valid codes. Case matters: `A11` is 2010-01-01, while `a11` is 2036-01-01.

Every code must describe an existing calendar date. `A2T` would mean 2010-02-29 and is rejected. `C2T` means 2012-02-29 and is valid. Zero is allowed in the year position, but neither month nor day can be zero.

No time, offset, time-zone name, or original input format is stored. Decoding recovers the calendar day and assigns midnight in the selected zone.

## cdate input and options

```text
./cdate [OPTIONS] [DATE OR TIMESTAMP ...]
```

Positional words are joined with spaces, so both `./cdate January 11, 2010` and `./cdate 'January 11, 2010'` work. Supply one date expression. `--date STRING` is an alternative to positional input.

| Option | Behavior |
| --- | --- |
| `-h`, `--help` | Print usage and all options, then succeed without reading reference files, running `date`, or creating logs. |
| `-V`, `--version` | Print `cdate 1.0.0` without operational side effects. |
| `-d`, `--debug` | Create a private execution log in the current directory. |
| `--date STRING` | Use the supplied date expression instead of positional words. |
| `-i`, `--input-format FORMAT` | Choose `auto`, `ymd`, `mdy`, `dmy`, `unix`, `unix-ms`, `date`, or `javascript`. Default: `auto`. |
| `-r`, `--reference FILE` | Encode the last modification instant of a file in the selected zone. |
| `-u`, `--utc`, `--universal` | Use UTC for unzoned input and for the resulting calendar date. |
| `-l`, `--local` | Explicitly use the inherited system/`TZ` local zone. |
| `-z`, `--timezone ZONE` | Use a named IANA zone such as `America/New_York` or `Asia/Kolkata`. |

Select at most one time-zone option. `--reference` cannot be combined with a date input; an explicit input format requires an input. Options are singletons, including aliases: `--utc -u` is an error. `--date` is long-only because `-d` means debug in these tools.

### Automatic detection

`auto` recognizes these forms before using the installed `date` parser for other expressions:

| Input | Interpretation |
| --- | --- |
| `2010-01-11`, `2010/01/11`, `2010.01.11` | Year, month, day in the selected zone. |
| `20100111` | Eight-digit `YYYYMMDD`. |
| `2010`, `2010-01` | First day of the ISO year or year-month in the selected zone. |
| `2010-01-11T13:41:54Z` | A UTC instant, then converted to the selected zone. |
| `2010-01-11T13:41:54+05:30` | An instant with an explicit numeric offset. |
| `2010-01-11T13:41:54.123456789Z` | Fractional seconds, with up to nine digits. |
| `2010-01-11T13+05`, `2010-01-11T13:41+05:30` | Hour/minute precision, including `date -Ihours` output. |
| `01/11/2010`, `01/11/10` | Month/day/year unless the first field exceeds 12. |
| `13/01/2010` | Day/month/year, since 13 cannot be a month. |
| `January 11, 2010`, `11 Jan 2010` | English calendar expressions handled by `date`. |
| `Mon Jan 11 13:41:54 UTC 2010` | Standard English `date` output. |
| `Mon, 11 Jan 2010 13:41:54 +0000` | RFC email output from `date -R`. |
| `Mon Jan 11 2010 13:41:54 GMT+0000 (Coordinated Universal Time)` | Standard JavaScript `Date.toString()` output. |
| `1/11/2010, 1:41:54 PM` | Common numeric JavaScript locale output, with a validated 12-hour clock. |
| `@1263217314` | Unix seconds, independent of digit count. |
| `1263217314` | Ten-digit Unix seconds. |
| `1263217314000` | Thirteen-digit Unix milliseconds. |

Two-digit years in numeric month/day or day/month input follow the usual `date` window: 00–68 mean 2000–2068, and 69–99 mean 1969–1999. The compact output range still applies, so years outside 2000–2061 fail.

Other input, such as `today`, `yesterday`, `tomorrow`, `next Friday`, or `'2010-01-11 +1 day'`, is handled by the installed `date` parser. Its accepted grammar and relative-date behavior can vary by implementation. Standard ISO 8601 and RFC 3339 output modes, RFC email output, JavaScript `toISOString()`, `toJSON()`, `toUTCString()`, `toString()`, and `toDateString()` are covered. Time-only expressions use the current day because they contain no historical calendar date.

For ambiguous input, select a format:

```sh
./cdate -i dmy 11/01/2010          # A1B, January 11
./cdate -i mdy 11/01/2010          # AB1, November 1
./cdate -i dmy 11012010            # A1B
./cdate -i mdy 01112010            # A1B
./cdate -i unix 1263217314         # A1B in UTC
./cdate -i unix-ms 1263217314000   # A1B in UTC
```

`ymd`, `mdy`, and `dmy` require numeric calendar input, optionally followed by a time and zone. `date` delegates the input grammar directly to the system command, bypassing automatic numeric interpretation. For example, the installed `date` can interpret bare `2010` as 20:10 today, while `auto` treats it as 2010-01-01.

`unix` accepts up to nine fractional digits; `unix-ms` accepts up to six, preserving nanosecond precision during conversion. Epoch values use exact integer arithmetic. Scientific notation, infinity, trailing junk, and excess precision are rejected. In `auto`, ten- or thirteen-digit timestamps may also include fractions. Use an explicit epoch mode for other digit counts.

### Local time, UTC, and JavaScript semantics

Unzoned calendar dates mean midnight in the selected zone. This keeps `./cdate 2010-01-11` on January 11 in any zone where that midnight exists. Unzoned clock times also use the selected zone. Explicit offsets identify an instant, and the selected zone determines which day gets encoded.

```sh
TZ=America/Los_Angeles ./cdate 2010-01-11T00:30:00Z      # A1A, January 10
TZ=America/Los_Angeles ./cdate -u 2010-01-11T00:30:00Z   # A1B, January 11
./cdate -z Asia/Kolkata 2010-01-11T23:30:00Z            # A1C, January 12
```

JavaScript has a different rule for ISO date-only strings: [`Date.parse` interprets them as UTC, while an ISO date-time without an offset uses local time](https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-date.parse). Choose `-i javascript` to apply those semantics. Calendar validity is checked independently before `Date.parse`, preventing permissive rollover of invalid dates.

```sh
TZ=America/Los_Angeles ./cdate 2010-01-11                 # A1B
TZ=America/Los_Angeles ./cdate -i javascript 2010-01-11   # A1A
```

Valid ISO `24:00`, with zero minutes, seconds, and fraction, denotes the end of a day and is normalized to the next day. `24:00:01`, leap-second notation, impossible calendar dates, and unzoned absolute times that round-trip to a different clock due to a DST gap are rejected. For repeated clock times during a DST fall-back, the system parser chooses an occurrence; use an explicit offset when the instant matters.

The final selected day determines representability. For example, `1999-12-31T23:30:00-01:00` becomes 2000-01-01 in UTC and can be encoded. Conversely, converting a boundary instant into another zone can put it outside the range.

### Locale limits

English month names and ordinary numeric locale strings are supported. Unicode direction marks and common nonbreaking spaces are removed from numeric locale output before parsing. Arbitrary localized month names, non-Gregorian calendars, alternate digit systems, and custom formats cannot be autodetected reliably. JavaScript itself [does not require `Date.parse` to round-trip `toLocaleString()`](https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-date.parse).

Use ISO output for interchange, or select `mdy`/`dmy` for a numeric locale date. Localized standard `date` output is accepted when the installed parser understands it. Setting `LC_ALL=C` when producing textual `date` output makes its English form predictable.

## ddate output and options

```text
./ddate [OPTIONS] CODE [+FORMAT]
./ddate [OPTIONS] --date CODE [+FORMAT]
```

Without formatting options, `ddate` prints the same default representation as `date`, at midnight on the decoded day. Use `+%F` for a stable `YYYY-MM-DD` result.

| Option | Behavior |
| --- | --- |
| `-h`, `--help` | Print help and succeed without running `date` or creating logs. |
| `-V`, `--version` | Print `ddate 1.0.0` without operational side effects. |
| `-d`, `--debug` | Create a private execution log in the current directory. |
| `--date CODE` | Supply the code instead of a positional argument. |
| `-u`, `--utc`, `--universal` | Interpret and format the decoded day at UTC midnight. |
| `-l`, `--local` | Explicitly use the system/`TZ` local zone. |
| `-z`, `--timezone ZONE` | Interpret and format midnight in a named IANA zone. |
| `-I`, `--iso-8601[=FMT]` | ISO 8601 output; `date`, `hours`, `minutes`, `seconds`, or `ns`. Bare `-I` means `date`. |
| `-R`, `--rfc-email`, `--rfc-2822` | RFC 5322 email output. |
| `--rfc-3339 FMT` | RFC 3339 output; `date`, `seconds`, or `ns`. |
| `+FORMAT` | The installed `date` command's custom output format. |

Attach an ISO precision with `=` or directly to `-I`, as in GNU `date` usage: `--iso-8601=seconds` or `-Iseconds`. Use only one output mode. Supplying two formats, an ISO option together with `+FORMAT`, or duplicate aliases fails.

```sh
./ddate A1B -I                         # 2010-01-11
./ddate -u A1B -Iseconds                # 2010-01-11T00:00:00+00:00
./ddate -u A1B --rfc-3339 seconds       # 2010-01-11 00:00:00+00:00
./ddate -u A1B -R                      # Mon, 11 Jan 2010 00:00:00 +0000
./ddate -u A1B '+%F %T %z'              # 2010-01-11 00:00:00 +0000
```

Percent directives, flags, widths, and locale modifiers go directly to `date`. This includes weekday/month names, week numbers, day of year, epoch seconds, numeric offsets, tabs, and newlines. See the installed `date --help` and the [GNU date format documentation](https://www.gnu.org/software/coreutils/manual/html_node/Date-format-specifiers.html). Availability of individual extensions follows the installed implementation.

Formatting time fields gives midnight; nanoseconds are zero. `%s` gives the epoch instant for midnight in the selected zone, which can differ between UTC and local time. `LC_ALL`, `LC_TIME`, and `LANG` are inherited for formatting.

`ddate` verifies that midnight exists and that the system parser returns the same calendar and clock. A date skipped by a zone change, such as 2011-12-30 in `Pacific/Apia`, fails in that zone and works with `-u`. UTC changes the interpretation of the decoded day; it cannot recover the source timestamp or its original zone.

These commands cover date conversion and display. They do not expose `date`'s system-clock setting flags, batch-file input, or clock-resolution query. `--reference` is available on `cdate` because a modification timestamp can be encoded.

## Errors, output, and debug logs

Normal stdout contains only the requested code or formatted result. Output ends with a newline, subject to additional newlines requested through `+FORMAT`. Errors and the debug-log path announcement go to stderr. Success exits with status 0; invalid input, unsupported dates, file failures, missing `date`, timeouts, formatting failures, and other errors exit with status 1.

Examples of rejected input:

```sh
./cdate 2010-02-30                     # Impossible calendar date
./cdate 1999-12-31                     # Before the supported range
./cdate 2062-01-01                     # After the supported range
./ddate A2T                           # February 29 in a non-leap year
./ddate A0B                           # Month zero
./ddate A1W                           # Day 32
./ddate A1B --unknown                  # Unknown option
```

Both commands use strict Node `parseArgs` parsing, reject duplicate singleton options, and flush standard-stream writes before returning. Inputs are passed to `date` as argument values through `execFile`, without shell interpolation. Each `date` subprocess has a five-second timeout and a one-MiB output limit. Neither command changes the system clock or makes network requests.

Enable diagnostic logging with `-d` or `--debug`:

```sh
./cdate --debug 2010-01-11
./ddate --debug A1B +%F
```

The log is named `_YYYYMMDD_HHMMSS_cdate.log` or `_YYYYMMDD_HHMMSS_ddate.log` in the current directory. Its timestamp uses local time, independently of the conversion zone. The executable basename determines the final name if a command is renamed. Files are created exclusively with mode `0600`; an existing same-second name causes an error and is never overwritten.

Logs record invocation metadata, the execution plan, input interpretation, parser calls and results, validation decisions and their reasons, completion, and failures with their causes. Structured details use two-space JSON. Debug records stay in the file; stderr receives only the path announcement and ordinary errors. Logs include input strings, paths, and formatting expressions. They do not collect credentials, full environment dumps, or HTTP headers; these commands have no HTTP mode.

Argument-parser failures happen before the log is opened. Help and version never create a log, including when combined with `--debug`. A log-write or log-close failure makes the command fail; a late failure can occur after the primary result has been written, so callers should check exit status.

## Development and verification

The first two lines form the shell/TypeScript bootstrap. `/bin/sh` opens the executable itself on stdin and replaces its process with `node --input-type=module-typescript -`, forwarding arguments unchanged. `STANDALONE_SCRIPT_NAME` preserves the invoked filename. The runtime strips erasable type syntax; it does not typecheck it.

Use the standalone CLI skill's bundled checker for strict validation. It obtains TypeScript and Node typings in an isolated `npx` environment, creates a private temporary `.ts` copy, checks it with strict erasable-syntax settings, and removes its temporary files. The first run may download the checker dependencies.

```sh
CHECKER="$HOME/.codex/skills/build-standalone-ts-cli/scripts/typecheck-cli"
timeout 60s "$CHECKER" ./cdate
timeout 60s "$CHECKER" ./ddate
timeout 60s node --test tests/date-tools.test.mjs
```

Both commands contain no `any` types. The tests exercise all 22,646 valid date round-trips and all 238,328 three-character combinations, compare formatting against the installed `date`, and cover the requested inputs, year transitions, invalid calendars, epoch precision, local/UTC behavior, DST gaps, reference metadata, argument errors, help without `date`, private debug logs, collision protection, and executable paths containing spaces. The test harness uses Node's type-stripping API to inspect the actual pure helpers; that API can emit an experimental warning independently of command execution.

Inspect the bootstrap separately when using ShellCheck. The executable's TypeScript body is not shell syntax and must be validated by the bundled checker. Do not run ShellCheck on the complete polyglot file. The extracted bootstrap passes with `SC2094` excluded: that rule mistakes the `STANDALONE_SCRIPT_NAME="$0"` environment assignment for a write to the source file, although the bootstrap only reads it.

The [offline interactive walkthrough](date-tools-walkthrough.html) explains the command flow with code excerpts, selectable help and terminal examples, a compact-date calculator, and the validation rules. Open it directly in a browser; it has no external assets and never executes the commands.
