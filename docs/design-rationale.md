# Design Rationale

## Why lenient parsing with warnings

Version 0.1.0 silently dropped properties that couldn't be fully parsed
(e.g., property tuples with fewer than 4 elements, value/type mismatches).
The `filter_map` in the deserializer returned `None` and the caller never
knew data was lost.

For NG9-1-1 use (EIDO embeds jCard for person/entity data), this is
unacceptable. NENA requires two things that are in tension:

1. **Present whatever you can to the calltaker** — a malformed property
   value must not prevent the calltaker from seeing other properties.
2. **Report data problems upstream** — malformed data triggers NENA
   discrepancy reports so the originating network can fix the issue.

Silent dropping satisfies neither: the calltaker doesn't see the data,
and the system has no structured information about what was wrong.

### The Parsed<T> pattern

```rust
pub struct Parsed<T> {
    pub value: T,
    pub warnings: Vec<ParseWarning>,
}
```

`JCard::from_json()` returns `Result<Parsed<JCard>, Error>` where `Err`
is reserved for structural failures (invalid JSON, missing "vcard" tag).
Data-level problems become warnings:

- A property tuple with 3 elements instead of 4 → warning with raw JSON,
  property dropped (can't extract a useful property without a value).
- Type/value mismatch (type "text" but value is a JSON number) → warning,
  value preserved as text so the calltaker can still see it.
- Parameters not a JSON object → warning, property preserved with empty
  parameters.

This pattern originates from the EIDO crate's handling of PIDF-LO and
ADR XML documents, where the same parse-and-warn approach proved
essential for production NG9-1-1 deployments.

### The serde Deserialize impl stays

The serde `Deserialize` impl calls the same internal lenient parser but
discards the warnings. Callers who use `serde_json::from_str::<JCard>()`
get the same best-effort parsing behavior — malformed properties are
handled the same way — but without access to the warning details. This
keeps the simple API path working for non-NENA consumers.

## Why an owned error type instead of exposing serde_json::Error

`JCard::from_json()` needs to report two kinds of failures: invalid JSON
(syntax error) and invalid jCard structure (valid JSON but not
`["vcard", [...]]`). The natural return type is an enum:

```rust
pub enum Error {
    InvalidJson(Box<dyn std::error::Error + Send + Sync>),
    InvalidStructure(String),
}
```

Using `Box<dyn Error>` instead of `serde_json::Error` avoids leaking a
dependency type in the public API. A `serde_json` major version bump
changes only jCard's internals rather than becoming a semver break for
downstream users. Callers still get `.to_string()` and `.source()` for
the original error — they just can't downcast to `serde_json::Error`,
which they shouldn't need to do.

This is stricter than what the EIDO crate currently does (EIDO exposes
`serde_json::Error` directly in `from_json()`), and an EIDO change
request has been filed to adopt the same pattern.

## Why value_type is stored separately on Property

RFC 7095 defines a set of type identifiers (text, uri, date, date-time,
etc.) plus extension types (x-types). The type identifier is the third
element of the property tuple and must be preserved exactly for
round-tripping.

Version 0.1.0 derived the type identifier from the `PropertyValue` enum
variant (e.g., `PropertyValue::Text` → "text"). This broke round-tripping
for extension types: `["x-foo", {}, "x-custom", "bar"]` would deserialize
with `value_type = "x-custom"` but `PropertyValue::Text` (fallback), and
`PropertyValue::default_type()` returns "text", not "x-custom".

Storing `value_type: String` on `Property` separately means serialization
uses the stored identifier, not the enum-derived one. The enum still
provides typed access to the value; the stored string ensures the wire
format is preserved.
