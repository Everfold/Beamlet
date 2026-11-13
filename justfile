set positional-arguments

beamletl_package := "beamletl"
default:
    @just --list

install-dev:
    cargo binstall prek cargo-deny cargo-nextest cargo-update cargo-about -y --force
    cargo deny fetch

beamletl *args:
    cargo run --package "{{beamletl_package}}" -- "$@"

build: build-beamletl

build-beamletl *args:
    cargo build --package "{{beamletl_package}}" "$@"

release: release-beamletl

release-beamletl *args:
    cargo build --release --package "{{beamletl_package}}" "$@"

check:
    cargo check --workspace

fmt:
    cargo fmt --all
    prek run --all-files

test:
    cargo nextest run --workspace

check-kernel-model:
    quint typecheck spec/kernel/tests.qnt
    quint test spec/kernel/tests.qnt
    quint run spec/kernel/tests.qnt --invariant=invariants --max-samples=1000 --max-steps=30 --seed=42 --verbosity=1
    quint verify spec/kernel/tests.qnt --backend=tlc --invariant=invariants --max-steps=10 --verbosity=1

doc:
    cargo doc --workspace --no-deps

license:
    cargo about generate --workspace about.hbs > licenses.html

update:
    cargo update
    cargo install-update -a
    cargo deny fetch
    prek autoupdate
    nix flake update
    npx skills update
