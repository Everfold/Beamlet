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

check-roles-model:
    quint typecheck spec/quint/roles/roles_test.qnt
    quint test spec/quint/roles/roles_test.qnt
    quint run spec/quint/roles/roles.qnt --main=roles --invariant=invariants --witnesses witnessAcceptedAndClosed witnessRetryWithSecondAttempt witnessFailureKeepsAwaiting witnessOfflineWorkerReportAccepted --max-samples=2000 --max-steps=20 --seed=42 --verbosity=1
    quint verify spec/quint/roles/roles.qnt --main=roles --backend=tlc --invariant=invariants --max-steps=8 --verbosity=1

check-routing-model:
    quint typecheck spec/quint/routing/routing_test.qnt
    quint test spec/quint/routing/routing_test.qnt
    quint run spec/quint/routing/routing.qnt --main=routing --invariant=invariants --witnesses witnessSelected witnessReconnectInvalidatesOldEpoch witnessReplayDoesNotExtendTtl witnessExactRejectedWhileOtherReady --max-samples=2000 --max-steps=20 --seed=42 --verbosity=1
    quint verify spec/quint/routing/routing.qnt --main=routing --backend=tlc --invariant=invariants --max-steps=8 --verbosity=1

check-spec: check-proto check-kernel-model check-ledger-model check-roles-model check-routing-model

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
