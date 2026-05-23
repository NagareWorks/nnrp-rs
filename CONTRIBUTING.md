# Contributing to nnrp-rs

This repository publishes Rust SDK crates and release assets, so contribution flow needs to stay predictable.

## Branch Strategy

The repository mainline is the stable branch for released or release-ready crate state. In the current private repository this branch is `master`; if the public repository is later renamed to `main`, the same rule applies to `main`.

`develop` is the version integration branch for active preview work once multi-branch release preparation starts. Until this repository needs a separate integration branch, preview3 implementation may continue directly on the private mainline.

Use short-lived topic branches for day-to-day work:

- `feature/<scope>-<topic>` for new capabilities
- `fix/<scope>-<topic>` for bug fixes
- `docs/<scope>-<topic>` for documentation-only changes
- `chore/<scope>-<topic>` for maintenance and tooling updates
- `release/<version>` only after `develop` or the private mainline is ready to freeze into a public crate release candidate

Rules:

- Branch from the latest `develop` when an active develop branch exists.
- Otherwise, branch from the private mainline.
- Branch from the stable mainline only for hotfixes against already released stable state.
- Keep topic branches focused on one slice of work.
- Rebase or merge from the target integration branch regularly if the branch stays open.
- Merge normal preview work back to `develop` when it exists, or to the private mainline while the repository remains single-branch.
- Do not push directly to protected branches; enforce this with a GitHub ruleset or branch protection rule when the repository is public.
- Do not publish crates directly from topic branches.

`release/<version>` branches are freeze branches. Cut them only when the version is feature-complete enough for stabilization passes, packaging rehearsals, or manual workflow runs. Keep release branches short-lived unless a published line needs explicit long-term maintenance.

After a release branch is cut:

- accept only release-blocking fixes, version metadata, package metadata, and release documentation on that branch
- merge accepted fixes back to `develop` or the private mainline
- tag the final release from the release branch or from the merged stable state, according to the release workflow
- delete the release branch after publication unless it represents an explicitly maintained LTS line

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

- target `develop` for normal preview work when it exists, the private mainline while the repository remains single-branch, `main` for stable hotfixes, or `release/<version>` only during an active release freeze
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

Public crate publication is gated through the `Release` workflow and should only happen from a short release tag or an explicit manual dispatch.

- `Release` runs on pushed `v*` tags and on manual `workflow_dispatch`; normal branch pushes must not publish GitHub releases or crates.io artifacts.
- Manual `workflow_dispatch` runs should leave external publishing disabled unless you intentionally enable `create_tag`; crate publication from an untagged ref is not allowed.
- Use the `release` GitHub environment for any publish-capable job.
- Set `CARGO_PUBLISH_MODE` on the `release` environment to `disabled` or `token`.
- If you use token-based crates.io publishing, store `CARGO_REGISTRY_TOKEN` as an environment secret on `release`.

## Review Guidelines

Review for:

- protocol and wire compatibility risk
- packaging and release regressions
- missing tests for changed behavior
- CI workflow correctness
- documentation drift when user-facing behavior changes

Do not start normal feature, fix, docs, or maintenance review while the PR still carries multiple commits.
