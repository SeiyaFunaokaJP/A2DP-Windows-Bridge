# Contributing to BlueCaster

Thank you for your interest in contributing to BlueCaster!

## Bug Reports & Feature Requests

Feel free to [open an issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new/choose) — you'll find a few simple forms there:

- **Bug report**: Something isn't working right.
- **Device compatibility report**: How BlueCaster works with your adapter and headphones. "It just works" reports are welcome too!
- **Feature request**: Ideas big or small.

You don't need to fill in every field. Rough reports are fine, and I'll ask if I need more details.

For connection problems, **Copy Diagnostics** on the **Diagnostics** screen puts the version, adapter, connection flow and recent log on the clipboard, ready to paste into the report.

## Building from Source

For detailed setup instructions, see the [GitHub Pages](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/) documentation.

### Quick Start

**Requirements:**

- Windows 10/11 (x64 or ARM64)
- Rust (stable, 1.90 or later)
- Visual Studio 2022 or later with C++ desktop workload (for the C libraries)
- Node.js 22 and pnpm (for the UI)
- Git

**Build:**

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
cargo build --release -p bluecaster-core-exe -p bluecaster-driver
cd app/src/ui && pnpm install && pnpm build && cd ../../..
cargo build --release -p bluecaster-ui
```

**Before a pull request:** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and in `app/src/ui` `pnpm exec tsc --noEmit` and `pnpm exec vitest run`; `pwsh ./build.ps1 -Test` builds and runs them all (with `cargo fmt --check`), plus `python tools/check_lang.py`. The design is described in [docs/v2-design.md](docs/v2-design.md) (Japanese).

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m "Add my feature"`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a [Pull Request](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/pulls)

## License

BlueCaster is dual-licensed under the [Apache License 2.0](LICENSE-APACHE) or the
[MIT License](LICENSE-MIT), at the user's option. Unless you explicitly state
otherwise, any contribution you submit is provided under the same dual license
(`Apache-2.0 OR MIT`), without any additional terms or conditions.
