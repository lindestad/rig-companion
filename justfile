set windows-shell := ["pwsh", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

run:
    cargo run --release --bin rig-companion

demo:
    cargo run --bin rig-companion -- --demo

check:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    cargo test --all-targets

build:
    cargo build --release

demo-check:
    cargo run --bin rigctl -- demo-check

# Run after committing to refresh the permanent installation.
release:
    .\scripts\release.ps1
