#!/bin/bash
# Pre-release checks: fmt, clippy, feature builds, docs, tests, semver,
# publish dry-run. Run on a clean master before scripts/release-tag.sh.

set -e

cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --all-features -- -D warnings
cargo check --features lenient-deserialize --all-targets
cargo check --features xcard --all-targets
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --no-deps --all-features
cargo test --release
cargo test --release --features lenient-deserialize
cargo test --release --features xcard
cargo test --release --all-features
cargo semver-checks check-release
cargo publish --dry-run

echo "Pre-release checks passed"
