## Summary

- What release or release-preparation change is included?

## Versioning

- Target version:
- Why this version is needed:

## Package Impact

- [ ] Crate metadata changed
- [ ] Crate contents changed
- [ ] Release workflow behavior changed

Describe the release-facing impact:

## Validation

- [ ] `cargo fmt --all --check`
- [ ] `cargo test --workspace`
- [ ] `cargo package -p nnrp-core --allow-dirty --no-verify`
- [ ] Release workflow assumptions were checked

Commands or workflow runs used:

```text

```

## Manual Registry Steps

- [ ] No manual registry work required
- [ ] crates.io state was reviewed
- [ ] GitHub Release asset expectations were reviewed

Notes:

## Checklist

- [ ] Branch name matches repository conventions
- [ ] Commit messages follow Conventional Commits
- [ ] PR is squashed to one commit unless this is necessary `release/<version>` branch work
- [ ] Release notes or docs were updated if needed
