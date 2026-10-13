#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$repo_dir"

checker="$HOME/.codex/skills/build-standalone-ts-cli/scripts/typecheck-cli"
if [[ ! -x "$checker" ]]; then
  printf 'TypeScript checker is missing: %s\n' "$checker" >&2
  exit 1
fi
if ! command -v cargo >/dev/null; then
  printf 'cargo must be on PATH; source your Rust environment first.\n' >&2
  exit 1
fi

timeout 10s shellcheck tests/run-tests.sh
timeout 60s "$checker" impl_ts/cdate
timeout 60s "$checker" impl_ts/ddate
timeout 60s cargo fmt --manifest-path impl_rust/Cargo.toml --check
timeout 60s cargo clippy --manifest-path impl_rust/Cargo.toml --all-targets --locked -- -D warnings
timeout 60s cargo test --manifest-path impl_rust/Cargo.toml --locked
timeout 60s cargo build --manifest-path impl_rust/Cargo.toml --release --locked
timeout 60s env DATE_TOOLS_IMPL=all node --test tests/date-tools.test.mjs impl_ts/tests/calendar.test.mjs
