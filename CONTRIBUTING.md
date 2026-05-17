# Contributing to nnrp-rs

This repository publishes Rust SDK crates and release assets, so contribution flow needs to stay predictable.

## Branch Strategy

`main` is the protected integration branch.

Use short-lived topic branches for day-to-day work:

- `feature/<scope>-<topic>` for new capabilities
- `fix/<scope>-<topic>` for bug fixes
- `docs/<scope>-<topic>` for documentation-only changes
- `chore/<scope>-<topic>` for maintenance and tooling updates
- `release/<version>` only when stabilizing a public crate release candidate

Rules:

- Branch from the latest `main`.
- Keep topic branches focused on one slice of work.
- Rebase or merge from `main` regularly if the branch stays open.
- Merge back to `main` through a pull request.
- Do not push directly to `main`; enforce this with a GitHub ruleset or branch protection rule.
- Do not publish crates directly from topic branches.

`release/<version>` branches are optional and should be used only when a version needs stabilization passes, packaging rehearsals, or manual workflow runs without publishing from `main`.

## Commit Message Convention

Use Conventional Commits.

Preferred forms:

- `feat: add frame validation helpers`
- `fix: reject invalid extension frame ordering`
- `docs: clarify cargo publish sequence`
- `chore: tighten CI toolchain setup`
- `test: add wire parser regression coverage`
- `refactor: simplify packet encode paths`

Rules:

- Keep the subject line imperative.
- Keep the first line concise.
- Use a scope only when it adds clarity.
- You can use multiple local commits while iterating, but normal PRs from `feature/*`, `fix/*`, `docs/*`, or `chore/*` branches must be squashed to exactly one commit before review.
- Only version-maintenance PRs that target or originate from `release/<version>` branches may keep multiple commits when that history is actually needed.

## Pull Request Expectations

Every PR should:

- target `main`
- use the default GitHub PR template that auto-loads on the PR page; specialized reference variants remain in `.github/PULL_REQUEST_TEMPLATE/` when you need to adapt the structure
- explain the user-facing or engineering motivation
- summarize the main crates or modules changed
- list the validation performed
- mention release impact when crate output changes
- contain exactly one commit before review unless it is a necessary `release/<version>` branch PR
- pass the `required-checks` GitHub Actions job before merge

PRs that violate the normal one-commit rule are not reviewed until they are squashed.

## Validation Expectations

Before opening or merging a PR, prefer the narrowest validation that proves the touched slice:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo package -p nnrp-core --allow-dirty --no-verify` when crate packaging output changed

PRs that affect CI, packaging, or release assets should include the exact command or workflow path used for validation.

## Versioning and Release Notes

Do not reuse a published crate version. If crate contents change after publication, create a new version.

When preparing a release PR:

- update the version source intentionally
- confirm crate metadata is correct
- confirm release assets have the expected names
- note any manual steps required on registries

## Review Guidelines

Review for:

- protocol and wire compatibility risk
- packaging and release regressions
- missing tests for changed behavior
- CI workflow correctness
- documentation drift when user-facing behavior changes

Do not start normal feature, fix, docs, or maintenance review while the PR still carries multiple commits.