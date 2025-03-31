# Contributing to CSV Scout

🎉 Thanks for your interest in contributing to **CSV Scout**!  
This project welcomes contributions of all kinds — features, fixes, documentation, tests, and ideas.

---

## 🧰 Project Setup

Make sure you have the following installed:

- [Rust](https://rust-lang.org/tools/install) (stable toolchain)
- [pre-commit](https://pre-commit.com/#install)

Install dependencies:

```bash
cargo build
```

---

## 🧼 Code Style

We use `rustfmt` for formatting. Please run:

```bash
cargo fmt --all
```

CI will also check formatting automatically.

---

## ✅ Linting & Testing

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

---

## 🧪 Commit Messages

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <short summary>
```

Examples:

- `feat(cli): add support for --delimiter option`
- `fix(sniffer): handle multi-character quotes`
- `chore: update dependencies`

### Install Git Hook for Local Validation

Run this once:

```bash
pre-commit install --hook-type commit-msg
```

This will validate commit messages automatically.

---

## 🚀 Releasing

Publishing to crates.io is done manually via GitHub Actions.

To trigger a release:

1. Go to [Actions → Manual Release](../../actions)
2. Click "Run workflow"
3. Choose release level: `patch`, `minor`, `major`, or set a version like `0.9.1`

This will:

- Bump the version
- Tag the release
- Publish to crates.io
- Create a GitHub Release with changelog via `git-cliff`

---

## 💬 Questions?

Open an issue or start a discussion — we're happy to help.
