# ci/

The scripts the CI workflows run, kept here so that you can run them
too. See "What still lives here" below.

To change CI, edit `.github/workflows/` in a reviewed pull request.
Session credentials push workflow files (admin `DECISIONS.md` ADR-8, as
amended 2026-09-24), so staging a workflow here first for a maintainer
to promote is optional. Sessions still cannot push tags, so a maintainer
pushes any tag that a tag-triggered workflow needs.

The amendment also asks for the same change in `tabnas/admin` wherever
admin keeps a copy of the workflow:

- If admin's `rollout/workflows/` holds a `jsonl__<file>` template
  for the workflow you changed, make the same edit there. Admin
  `scripts/verify.sh` compares each template with its deployed copy, and
  a maintainer's `rollout/apply-workflows.sh --apply` would push the
  older text back over yours.
- `clib.yml` and `clib-release.yml` are stamped from admin
  `tasks/clib-template/` and carry a `tabnas-clib-template` marker.
  Change the template, then restamp with admin `tasks/adopt-clib.sh`,
  which writes both workflows straight into `.github/workflows/`. The
  new stamp lands in this repository's own reviewed pull request. Never
  edit the copies here.

## Promoted, 2026-09-22

Both files that were staged here are now live, moved by the rollout
script rather than edited: `workflows/docs.yml` is
`.github/workflows/docs.yml` and `workflows/rust.yml` is
`.github/workflows/rust.yml`. Nothing is pending. Read the workflows
themselves rather than a description of them here.

`rust.yml` opened with a header calling itself PROPOSED and telling the
reader to move it into `.github/workflows/`, which is where it already
was: the rollout moves files and does not rewrite their comments. The
header was corrected in place once ADR-8's amendment let a session edit
the live file.

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
