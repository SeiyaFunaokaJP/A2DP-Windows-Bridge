# Contributing to A2DP Windows Bridge (A2DPWB)

Thank you for your interest in contributing to A2DP Windows Bridge!

## Bug Reports & Feature Requests

Feel free to [open an issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new/choose) — you'll find a few simple forms there:

- **Bug report**: Something isn't working right.
- **Device compatibility report**: How A2DPWB works with your adapter and headphones. "It just works" reports are welcome too!
- **Feature request**: Ideas big or small.

You don't need to fill in every field. Rough reports are fine, and I'll ask if I need more details.

## Building from Source

For detailed setup instructions, see the [GitHub Pages](https://seiyafunaokajp.github.io/A2DP-Windows-Bridge/) documentation.

### Quick Start

**Requirements:**

- Windows 10/11 (x64)
- Visual Studio 2022 or later with C++ desktop workload
- CMake 3.16+
- Git

**Build:**

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
cmake -B build -A x64
cmake --build build --config Release
```

> The first build takes several minutes because CMake FetchContent downloads and compiles wxWidgets.

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m "Add my feature"`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a [Pull Request](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/pulls)

## License

Contributions are provided under the [MIT License](LICENSE).
