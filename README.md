# jcard

[![CI](https://github.com/ticpu/jcard/actions/workflows/ci.yml/badge.svg)](https://github.com/ticpu/jcard/actions/workflows/ci.yml)
[![docs.rs](https://docs.rs/jcard/badge.svg)](https://docs.rs/jcard)
[![crates.io](https://img.shields.io/crates/v/jcard.svg)](https://crates.io/crates/jcard)

[RFC 7095](https://www.rfc-editor.org/rfc/rfc7095) jCard (JSON representation
of vCard) types with serde support.

All value types from RFC 7095 §3.5 are supported: text, uri, date, time,
date-time, date-and-or-time, timestamp, boolean, integer, float,
utc-offset, language-tag, and unknown. Structured property values with
nested arrays (§3.3.1.3) are handled. Type identifiers are stored
separately for round-trip fidelity, including extension types.

## Usage

```toml
[dependencies]
jcard = "0.3"
```

### Builder

```rust
use jcard::JCard;

let jcard = JCard::builder()
    .fn_("Jane Doe")
    .n("Doe", "Jane", "", "", "")
    .email("jane.doe@example.com")
    .build();

let json = serde_json::to_string_pretty(&jcard).unwrap();
let parsed: JCard = serde_json::from_str(&json).unwrap();
assert_eq!(jcard, parsed);
```

### Lenient parsing with warnings

```rust
use jcard::JCard;

let parsed = JCard::from_json(r#"["vcard",[
    ["version",{},"text","4.0"],
    ["fn",{},"text","Jane Doe"],
    ["email","bad-params","text","jane@example.com"]
]]"#).unwrap();

// Best-effort result is always available
assert_eq!(parsed.value.properties().len(), 3);

// Warnings report what went wrong
for w in &parsed.warnings {
    eprintln!("{w}");
}
```

### From an existing serde_json::Value

```rust
use jcard::JCard;

let value: serde_json::Value = serde_json::from_str(
    r#"["vcard",[["version",{},"text","4.0"],["fn",{},"text","Test"]]]"#
).unwrap();

let parsed = JCard::from_value(&value).unwrap();
assert!(!parsed.has_warnings());
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
