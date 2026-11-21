set positional-arguments

console_package := "beamlet-console"
node_package := "beamlet-node"

default:
    @just --list

install-dev:
    cargo binstall prek cargo-deny cargo-nextest cargo-update cargo-about diesel_cli -y --force
    cargo deny fetch

console:
    cargo run --package "{{console_package}}"

node:
    cargo run --package "{{node_package}}"

build: build-console build-node

build-console *args:
    cargo build --package "{{console_package}}" "$@"

build-node *args:
    cargo build --package "{{node_package}}" "$@"

release: release-console release-node

release-console *args:
    cargo build --release --package "{{console_package}}" "$@"

release-node *args:
    cargo build --release --package "{{node_package}}" "$@"

check:
    cargo check --workspace

fmt:
    cargo fmt --all
    prek run --all-files

test:
    cargo nextest run --workspace

check-proto:
    buf format -d --exit-code spec/proto
    buf lint spec/proto
    buf build spec/proto -o /dev/null

check-kernel-model:
    quint typecheck spec/quint/kernel/kernel_test.qnt
    quint test spec/quint/kernel/kernel_test.qnt
    quint run spec/quint/kernel/kernel.qnt --main=kernel --invariant=invariants --witnesses witnessAttemptCreated witnessReportAccepted witnessClosed witnessRejectedMismatch --max-samples=1000 --max-steps=30 --seed=42 --verbosity=1
    quint verify spec/quint/kernel/kernel.qnt --main=kernel --backend=tlc --invariant=invariants --max-steps=10 --verbosity=1

check-ledger-model:
    quint typecheck spec/quint/ledger/ledger_test.qnt
    quint test spec/quint/ledger/ledger_test.qnt
    quint run spec/quint/ledger/ledger.qnt --main=ledger --invariant=invariants --witnesses witnessTwoOpsActive witnessAttemptReuseRejected witnessRetryWithFreshAttempt --max-samples=1000 --max-steps=20 --seed=42 --verbosity=1
    quint verify spec/quint/ledger/ledger.qnt --main=ledger --backend=tlc --invariant=invariants --max-steps=8 --verbosity=1

check-protocol-models:
    quint typecheck spec/quint/protocol/registry_lease_test.qnt
    quint test spec/quint/protocol/registry_lease_test.qnt
    quint run spec/quint/protocol/registry_lease.qnt --main=registry_lease --invariant=invariants --witnesses witnessSelected witnessReconnectInvalidatesOldEpoch witnessOfflineBlocksOldReady witnessMultiSourceFailover --max-samples=2000 --max-steps=20 --seed=42 --verbosity=1
    quint verify spec/quint/protocol/registry_lease.qnt --main=registry_lease --backend=tlc --invariant=invariants --max-steps=8 --verbosity=1
    quint typecheck spec/quint/protocol/delivery_test.qnt
    quint test spec/quint/protocol/delivery_test.qnt
    quint run spec/quint/protocol/delivery.qnt --main=delivery --invariant=invariants --witnesses witnessExactSuccess witnessExactNoFallback witnessTimeoutUnresolved witnessLateReportAfterWorkerOffline --max-samples=2000 --max-steps=20 --seed=42 --verbosity=1
    quint verify spec/quint/protocol/delivery.qnt --main=delivery --backend=tlc --invariant=invariants --max-steps=8 --verbosity=1

check-models: check-kernel-model check-ledger-model check-protocol-models

check-spec: check-proto check-models

doc:
    cargo doc --workspace --no-deps --document-private-items --open

license:
    cargo about generate --workspace about.hbs > licenses.html

update:
    cargo update
    cargo install-update -a
    cargo deny fetch
    prek autoupdate
    nix flake update
    npx skills update
