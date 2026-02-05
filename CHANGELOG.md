# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **BREAKING**: Container commands using system containerd (with sudo) now require explicit opt-in via `BN_ALLOW_SUDO=1` environment variable. This security change prevents accidental sudo usage. Use rootless containerd (recommended) or set `export BN_ALLOW_SUDO=1` to use system containerd.
