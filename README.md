# jcard

RFC 7095 jCard (JSON representation of vCard) types with serde support.

All value types from RFC 7095 §3.5 are supported: text, uri, date, time,
date-time, date-and-or-time, timestamp, boolean, integer, float,
utc-offset, language-tag, and unknown. Structured property values with
nested arrays (§3.3.1.3) are handled. Type identifiers are stored
separately for round-trip fidelity, including extension types.

## Usage

```toml
[dependencies]
jcard = "0.1"
```

```rust
use jcard::{JCard, PropertyValue};

let jcard = JCard::builder()
    .fn_("Jane Doe")
    .n("Doe", "Jane", "", "", "")
    .email("jane.doe@example.com")
    .build();

let json = serde_json::to_string_pretty(&jcard).unwrap();
let parsed: JCard = serde_json::from_str(&json).unwrap();
assert_eq!(jcard, parsed);
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
