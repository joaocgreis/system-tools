# Repository context and instructions

This repository provides `cdate` and `ddate` in TypeScript and Rust. A compact date stores one character each for the year, month, and day using `0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz`. The supported Gregorian calendar range is 2000-01-01 through 2061-12-31. `A1B` means 2010-01-11. Invalid calendars and dates outside the output range must fail without a result.

## Layout

- `impl_ts/` contains the independent extensionless TypeScript executables and TypeScript-specific tests and documentation.
- `impl_rust/` contains the Rust Cargo package, shared Rust modules, two binary entry points, and Rust-specific tests and documentation.
- `tests/` contains the shared CLI compatibility suite and its runner.
- `docs/` contains the shared command help text.
- `man/` contains the shared section 1 manual pages.
- `README.md`, `assets/`, and `date-tools-walkthrough.html` explain the common format and command behavior.

## Compatibility requirements

Keep tests in sync for all implementations. Add or change a shared CLI case once in `tests/date-tools.test.mjs`, then run it against every implementation. Do not copy that suite into implementation folders, silently skip an implementation, or weaken shared assertions to accommodate a port. Implementation-specific unit tests supplement the shared suite. Keep exhaustive calendar checks equivalent across languages.

Preserve matching options, successful output, exit statuses, validation, and time-zone semantics. The exact wording of error details and debug metadata may differ. Help text must match `docs/cdate-help.txt` and `docs/ddate-help.txt`; TypeScript embeds a copy to remain standalone, and Rust includes the shared files at compile time. Update those copies, the README, both man pages, and walkthrough help together when the interface changes.

Input and output zones are independent. Output defaults to UTC; `--local` affects output only. Unzoned `cdate` input defaults to UTC, while explicit offsets determine their own instant. For `ddate`, an explicit input-zone option overrides a zone supplied with the code; otherwise use the supplied zone, then UTC. Input overrides reinterpret the decoded calendar day at midnight. Validate missing clock times in the input zone before converting to the output zone.

Both implementations may call the installed GNU-compatible `date` for parsing, time-zone conversion, and formatting. Invoke it without shell interpolation, bound subprocess time and output, and propagate failures. Neither tool sets the clock or makes network requests. Keep normal stdout limited to the result. Flush output and report failures on stderr. Debug logs use exclusive creation, mode `0600`, and `_YYYYMMDD_HHMMSS_SCRIPTNAME.log` in the current directory; never overwrite a log or print its contents during routine tests.

## Development

Use `python3`, not `python`. Give shell commands a short, reasonable timeout. Run ShellCheck on Bash scripts. Never run ShellCheck on a complete TypeScript polyglot executable; inspect its fixed bootstrap separately and use the bundled TypeScript checker for its body. Do not invoke Git while working under the standalone TypeScript CLI skill.

Keep TypeScript executables independent of a package manifest and runtime transpiler. Preserve their native stdin TypeScript bootstrap, executable mode, strict `parseArgs` parsing, and absence of `any` types. Run the skill's bundled checker after editing either executable and before executing the changed file.

Keep Rust dependencies small and avoid unsafe code. Optimize release binaries for size first, then speed. Use size optimization, link-time optimization, one codegen unit, stripped symbols, and aborting panics in the release profile. Measure binary sizes when changing dependencies or optimization settings. Commit `Cargo.lock` as source material; keep generated binaries under the ignored `impl_rust/target/` directory. Do not remove guards to reduce size.

Run `tests/run-tests.sh` from the repository root for required checks. It must check the Bash runner, build Rust, run formatting and lint checks, validate TypeScript, and execute the shared suite for both implementations plus language-specific tests. Set an implementation selector only for focused development; final verification must cover both. Validate both man pages with `mandoc -T lint -W all man/cdate.1 man/ddate.1` when mandoc is available.

Read `~/HEADLESS_CHROMIUM.md` before browser testing and `~/TTS-KOKORO-AGENTS.md` before speech work. Prefer the T3 collaborative preview tools when available. Keep the walkthrough offline, accessible, and synchronized with the TypeScript excerpts it displays. Use plain documentation prose and retain the infographic and opening explanation at the start of the README.
