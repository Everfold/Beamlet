# Beamlet

**WIP**, with broken changes before the first release.

## Usage

### Nix

```bash
# List
nix flake show github:Everfold/Beamlet

nix run github:Everfold/Beamlet#beamletl -- --version
```

## Build

```bash
# Linux
apt install rustup \
  cargo-binstall \
  just

# Nix
nix develop

# Dev Tools
just install-dev

# Run
just beamletl

# Build
just build

# Release Build
just release

# Check
just check

# Format
prek install
just fmt

# Test
just test

# Doc
just doc

# Generate licenses.html
just license

# Update
just update

# Quint Check
just check-kernel-model
```
