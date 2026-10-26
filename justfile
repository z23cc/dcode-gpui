default: check

check: lint-source secrets fmt-check clippy test

secrets:
    gitleaks git --pre-commit --staged --redact --no-banner .
    gitleaks git --pre-commit --redact --no-banner .

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --locked -- -D warnings

test:
    cargo test --workspace --locked

lint-source:
    scripts/lint-source.sh

fix:
    cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged
    cargo fmt --all

hooks:
    git config core.hooksPath .githooks

run:
    cargo run -p dcode-app

probe:
    cargo run -p dcode-client --example probe

manifest ts_checkout:
    bun scripts/dump-manifest.ts {{ts_checkout}}
