# Rust implementation

This Cargo package builds native `cdate` and `ddate` executables with the interface described in the [shared README](../README.md). The installed binaries need a GNU-compatible `date` and time-zone data; they have no Node.js or TypeScript runtime dependency. The source targets Unix systems.

Build from the repository root with Rust 1.85 or newer:

```sh
timeout 60s cargo build --manifest-path impl_rust/Cargo.toml --release --locked
./impl_rust/target/release/cdate 2010-01-11
./impl_rust/target/release/ddate A1B +%F
```

If Cargo is absent from `PATH`, load your Rust environment or Bash profile before building. The first build downloads the two dependencies pinned in `Cargo.lock`.

To install the release binaries:

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 impl_rust/target/release/cdate impl_rust/target/release/ddate "$HOME/.local/bin/"
```

That replaces commands with the same names in the destination. Preserve an older installation first if you need it. Installing the man pages is described in the shared README.

Both entry points use one library for argument handling, Gregorian validation, input normalization, exact timestamp arithmetic, subprocess handling, and private logging. `lexopt` handles command-line tokens. `regex-lite` provides the ASCII patterns used for date recognition without a full Unicode regex engine. There are no other third-party dependencies and no unsafe code.

The [release profile](Cargo.toml) prioritizes size: size optimization, link-time optimization, one codegen unit, stripped symbols, and aborting panics. Guards and error reporting remain enabled. Cargo's [profile documentation](https://doc.rust-lang.org/cargo/reference/profiles.html) describes these settings. Generated files stay under the ignored `target/` directory. Distribute release binaries, rather than debug builds; binary sizes depend on the Rust version, target, and linker.

On the current x86_64 Linux environment with Rust 1.99.0, the stripped release binaries are 509,248 bytes for `cdate` and 493,752 bytes for `ddate`. An otherwise identical build with `opt-level = "s"` produced 515,688 and 498,208 bytes, so the release profile retains `"z"`. Repeat the comparison when changing the compiler or dependencies.

Parsing and formatting use the installed `date`, called directly with argument values. Both output pipes are drained concurrently, with a five-second timeout and a one-MiB limit per pipe. An explicit input offset is parsed independently of the default zone. Unzoned numeric dates and decoded midnight are checked against the parser's returned civil clock to reject skipped times.

JavaScript mode implements ISO date-only UTC semantics, the selected input zone for unzoned date-times, standard English JavaScript date strings, and millisecond precision without starting a JavaScript engine. Nonstandard legacy `Date.parse` grammar is implementation-defined in JavaScript itself; use ISO input, automatic mode, or explicit numeric formats for predictable interchange.

Run focused Rust checks with:

```sh
timeout 60s cargo fmt --manifest-path impl_rust/Cargo.toml --check
timeout 60s cargo clippy --manifest-path impl_rust/Cargo.toml --all-targets --locked -- -D warnings
timeout 60s cargo test --manifest-path impl_rust/Cargo.toml --locked
timeout 60s cargo build --manifest-path impl_rust/Cargo.toml --release --locked
timeout 60s env DATE_TOOLS_IMPL=rust node --test tests/date-tools.test.mjs
```

The shared CLI suite requires Node.js for its test harness, even when testing only Rust. Rust unit tests cover every valid calendar day, every three-character code, and negative and fractional epoch arithmetic. Use `tests/run-tests.sh` for final verification of both implementations. Keep shared cases in sync and add implementation-specific unit tests only for internal behavior.
