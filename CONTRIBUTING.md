# Contributing to A2DPWB

Thank you for your interest in contributing to A2DPWB!

## Bug Reports & Feature Requests

Please use [GitHub Issues](https://github.com/SeiyaFunaokaJP/A2DPWB/issues) for bug reports and feature requests.

- **Bug reports**: Include steps to reproduce, expected behavior, and actual behavior.
- **Feature requests**: Describe the feature you'd like and why it would be useful.

## Building from Source

For detailed setup instructions, see the [GitHub Pages](https://seiyafunaokajp.github.io/A2DPWB/) documentation.

### Quick Start

**Requirements:**

- Windows 10/11 (x64)
- Visual Studio 2022 or later with C++ desktop workload
- CMake 3.16+
- Git

**Build:**

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DPWB.git
cd A2DPWB
cmake -B build -A x64
cmake --build build --config Release
```

> The first build takes several minutes because CMake FetchContent downloads and compiles wxWidgets.

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m "Add my feature"`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a [Pull Request](https://github.com/SeiyaFunaokaJP/A2DPWB/pulls)

## License

Contributions are provided under the [MIT License](LICENSE).
