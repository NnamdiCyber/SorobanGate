## Description

Please include a summary of the change and which issue it fixes.

Fixes #(issue)

## Type of Change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Performance improvement
- [ ] Refactoring (no functional changes)
- [ ] Test addition or improvement
- [ ] Documentation update
- [ ] Chore (tooling, CI, dependencies)

## Checklist

- [ ] I have run `cargo fmt --all -- --check`
- [ ] I have run `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] I have added or updated tests in `tests/integration/`
- [ ] New public API items have doc comments (`///`)
- [ ] `unsafe` blocks have `// SAFETY:` comments
- [ ] No panics in the hot path — errors use `anyhow` / `thiserror`
- [ ] My commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)

## How Has This Been Tested?

Describe the tests you ran and their outcomes.

## Additional Context

Any relevant logs, metrics, or screenshots.
