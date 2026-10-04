# Contributing to audiflow

Thank you for considering contributing to the audiflow project! This document
explains how to contribute to any repository in the audiflow ecosystem.

## Contributor License Agreement (CLA)

All contributors must sign our [Contributor License Agreement](CLA.md) before
their first pull request can be merged. This is a one-time process.

**Why do we require a CLA?**

The audiflow project uses AGPL-3.0-or-later for code and CC BY-SA 4.0 for data. The CLA
ensures that the project maintainer can continue to offer the project under these
licenses and, in the future, potentially under additional commercial licenses.
Without a CLA, every contributor retains copyright over their contributions,
making it legally impossible to change licensing terms -- even when doing so would
benefit the project and its community.

The CLA does NOT transfer your copyright. You retain ownership of your
contributions. It grants the project a broad license to use your contributions
in a way that keeps the project sustainable long-term.

**How to sign:**

1. Submit your pull request
2. A bot will comment asking you to sign
3. Reply with: `I have read the CLA Document and I hereby sign the CLA`
4. Done! This applies to all future contributions across audiflow repos

## Repositories

| Repository | Content | License |
|------------|---------|---------|
| `audiflow` | Flutter mobile app | AGPL-3.0-or-later |
| `audiflow-preset` | Playlist config data | CC BY-SA 4.0 |
| `audiflow-preset-editor` | Web editor (Rust + React) | AGPL-3.0-or-later |

## How to Contribute

### Reporting Issues

- Use the GitHub Issues tab in the relevant repository
- Search existing issues before creating a new one
- Include steps to reproduce for bugs

### Code Contributions

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Ensure tests pass
5. Submit a pull request

### Playlist Data Contributions

The `audiflow-preset` repository contains curated podcast playlists.
Contributions of new playlists or improvements to existing ones are welcome.

1. Fork `audiflow-preset`
2. Use the [audiflow-preset-editor](https://github.com/audiflow/audiflow-preset-editor)
   to create or modify playlists
3. Export the config data
4. Submit a pull request

### Code Style

- Follow existing patterns in the codebase
- Keep commits atomic and focused
- Use conventional commit messages (`feat:`, `fix:`, `chore:`, etc.)

## Questions?

Open an issue in the relevant repository or start a discussion.
