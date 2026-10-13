# TypeScript implementation

`cdate` and `ddate` are independent, extensionless executable TypeScript files. Their interface and compact format are described in the [shared README](../README.md). Each file can be copied and run on its own.

Run them from the repository root:

```sh
./impl_ts/cdate 2010-01-11
./impl_ts/ddate A1B +%F
```

The runtime requires Node.js 22.18 or newer on `PATH`, `/bin/sh`, a GNU-compatible `date`, and time-zone data. There is no package manifest, build step, runtime transpiler, or third-party runtime dependency. Development typechecks use `npx`.

To install this implementation:

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 impl_ts/cdate impl_ts/ddate "$HOME/.local/bin/"
```

That replaces commands with the same names in the destination. Preserve an older installation first if you need it.

The first two lines form a shell/TypeScript bootstrap. `/bin/sh` opens the executable itself on stdin and replaces its process with `node --input-type=module-typescript -`, forwarding arguments unchanged. `STANDALONE_SCRIPT_NAME` preserves the invoked filename. Node strips erasable type syntax; it does not typecheck it.

Use the standalone CLI skill's bundled checker after editing either executable and before running it. The checker obtains TypeScript and Node typings in an isolated `npx` environment, creates a private temporary `.ts` copy, checks strict erasable syntax, and removes its temporary files. The first run may download dependencies.

```sh
checker="$HOME/.codex/skills/build-standalone-ts-cli/scripts/typecheck-cli"
timeout 60s "$checker" impl_ts/cdate
timeout 60s "$checker" impl_ts/ddate
timeout 60s env DATE_TOOLS_IMPL=ts node --test tests/date-tools.test.mjs impl_ts/tests/calendar.test.mjs
```

Both commands contain no `any` types. The unit tests extract the actual pure calendar helpers using Node's type-stripping API and check all 22,646 valid days and all 238,328 possible three-character strings. That API may emit an experimental warning independently of command execution. CLI behavior lives in the shared suite; add a case there once and run it against both implementations with `tests/run-tests.sh`.

ShellCheck applies to the extracted bootstrap, not the complete polyglot source. The TypeScript body must be checked by the bundled checker. Exclude `SC2094` for the bootstrap: it mistakes the `STANDALONE_SCRIPT_NAME="$0"` environment assignment for a write to the source file, although the bootstrap only reads it.

The TypeScript implementation uses strict Node `parseArgs`, `execFile` for bounded `date` calls, and native `Date.parse` in explicit JavaScript mode. Keep each executable self-contained, including its embedded copy of the shared help text. The [offline walkthrough](../date-tools-walkthrough.html) displays excerpts from these files.
