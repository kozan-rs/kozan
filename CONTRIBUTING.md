# Contributing to Kozan

Thanks for your interest in Kozan.

## Before you start

Open an issue before writing code. The architecture is opinionated and moves fast — a quick conversation upfront saves everyone time.

## Getting started

```bash
git clone https://github.com/kozan-rs/kozan.git
cd kozan
cargo build --workspace
cargo test --workspace
```

Rust 1.85+ is required (edition 2024).

## Making changes

1. Fork the repo and create a branch from `main`.
2. Write your code. Follow the style of the surrounding code — no special formatting rules beyond `rustfmt` and the workspace clippy lints.
3. Add tests if your change is testable.
4. Make sure `cargo test --workspace` passes.
5. Open a pull request against `main`.

## What we look for in PRs

- Does it solve a real problem?
- Is the approach consistent with the existing architecture?
- Are there tests?
- Is the diff minimal — no unrelated cleanups mixed in?

## Reporting bugs

Open an issue with:
- What you did
- What you expected
- What happened instead
- Rust version and OS

## Code of conduct

Be respectful. That's the whole policy.
