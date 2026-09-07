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

`.claude/commands/release.md` (`/release`) is the canonical process:
`scripts/release-check.sh`, changelog to `scratch/`, `scripts/release-tag.sh`
(Cargo.lock pinned on the detached tag commit only — never on master), push
and wait for CI green, then `cargo publish` run directly — publish is
irrevocable and is never wrapped in a script. Never publish without the tag
pushed and CI green on master.

Install the hooks once per clone: `hooks/install.sh`.

## RFC Reference Texts

`rfc/` is gitignored, so the specs stay out of the repo and out of the
published package. Fetch them there when needed:

```sh
mkdir -p rfc
curl -o rfc/rfc7095.txt https://www.rfc-editor.org/rfc/rfc7095.txt
curl -o rfc/rfc6351.txt https://www.rfc-editor.org/rfc/rfc6351.txt
```

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
