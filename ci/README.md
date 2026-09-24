# ci/

Staging area for GitHub Actions workflow changes.

This directory exists because session credentials cannot write
`.github/workflows/*` — see admin `DECISIONS.md` ADR-8. To change CI:

1. Put the intended workflow file in `workflows/`.
2. A maintainer promotes it with the admin `rollout/apply-ci-folders.sh`
   script.

## Promoted, 2026-09-22

Both files that were staged here are now live, moved by the rollout
script rather than edited: `workflows/docs.yml` is
`.github/workflows/docs.yml` and `workflows/rust.yml` is
`.github/workflows/rust.yml`. Nothing is pending. Read the workflows
themselves rather than a description of them here.

`rust.yml` still opens with a header calling itself PROPOSED and telling
the reader to move it into `.github/workflows/`, which is where it
already is. The rollout moves files and does not rewrite their comments,
and session credentials cannot push `.github/workflows/*` to correct it,
so the fix is a staged copy here and another rollout run.

## What still lives here

- **`rust/run.sh`** is the Rust gate itself: `rs/` built, tested,
  `rustfmt`-checked, and clippy-clean at `-D warnings`.
  `.github/workflows/rust.yml` calls it and you can run it too;
  `make test-rs` stays the fast inner loop.

  The workflow clones the siblings `tabnas/parser`, `tabnas/json`, and
  `tabnas/support` itself, because `rs/Cargo.toml` takes all three as
  path dependencies. That is also why the gate does **not** pass
  `--locked`: the lock records each sibling by version, so once any of
  them bumps, `--locked` would fail every pull request here, including
  ones touching no Rust. See the comment in `ci/rust/run.sh`.
