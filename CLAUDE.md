## Project Type

RFC 7095 jCard (JSON representation of vCard) parser and builder library.

## No PII or Organization-Specific Data

**NEVER** include real phone numbers, real hostnames, organization names,
internal URLs, or any other PII in source code, tests, or documentation.
Use RFC-compliant test values only:

- Phone numbers: `+1555xxxxxxx` (555 prefix)
- Domains: `example.com`, `example.org`, `example.net` (RFC 6761)
- Organization names: "EXAMPLE CO", generic descriptions
- Email: `user@example.com` (RFC 2606)

The pre-commit hook runs gitleaks to enforce this.

## Release Workflow

### Pre-release checks

```sh
cargo fmt --all
cargo clippy --release -- -D warnings
cargo test --release
cargo build --release
cargo semver-checks check-release
cargo publish --dry-run
```

### Publish

**Never `cargo publish` without completing these steps first:**

1. Create signed annotated tags (`git tag -as`)
2. Push the tags (`git push --tags`)
3. Wait for CI to pass on the tagged commit
4. Only then `cargo publish`

## Build & Test

```sh
cargo fmt --all
cargo check --message-format=short
cargo clippy --fix --allow-dirty --message-format=short
cargo test
```

## Key Design Decisions

- Serde-based serialization/deserialization matching RFC 7095 JSON array format
- Property tuple: `[name, parameters, value_type, value]`
- Builder pattern for ergonomic construction
- `Display`/`FromStr` for string conversion
- BTreeMap for parameters (deterministic serialization order)
- PropertyValue enum covers text, uri, boolean, integer, float
- TextList variant for structured values (e.g. N with name components)
