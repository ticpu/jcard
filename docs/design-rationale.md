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

### The serde Deserialize impl and the lenient-deserialize feature

The serde `Deserialize` trait has a fixed signature: `Result<Self, D::Error>`.
There is no side channel for warnings. This creates a tension between two
legitimate use cases:

**Standalone users** call `serde_json::from_str::<JCard>(s)` and expect
`Err` when the input is structurally invalid. Returning `Ok(default)`
silently is an implicit policy decision — the library hides the failure
and the caller has no signal that anything went wrong.

**Embedding users** (EIDO) have `Option<JCard>` as a field in a parent
struct. When `Deserialize` returns `Err`, serde propagates it and the
entire parent deserialization fails. A malformed contact card should not
prevent a calltaker from seeing the emergency incident data.

Neither behavior is universally correct, and unconditional leniency
violates the project's "transparent, not clever" principle. The solution
is a feature gate:

- **Default (no feature):** `Deserialize` returns `Err` on structural
  failure. Honest — the caller sees the error and decides what to do.
- **`lenient-deserialize`:** `Deserialize` returns `JCard::default()`
  (a minimal jCard with only the mandatory `version` property) on
  structural failure. The error is silently absorbed — callers who need
  diagnostics should use `from_json()` or `from_value()`.

The EIDO crate enables `lenient-deserialize` so that jCard fields
degrade gracefully. It then calls `JCard::from_value()` on the raw JSON
value in a second pass to collect warnings with proper field path
prefixes. Standalone users get strict deserialization by default and are
never surprised by silent empty jCards.

### from_value() avoids roundtripping

`from_json()` accepts `&str`, parses it into a `serde_json::Value`, then
runs the lenient parser. When a caller already holds a `serde_json::Value`
— typically extracted from a parent JSON document — they would have to
roundtrip through `serde_json::to_string()` + `from_json()` just to get
warning collection. That's an unnecessary allocation and reparse.

`from_value(&serde_json::Value)` exposes the internal parser directly.
`from_json()` becomes a thin wrapper that handles the JSON string → Value
step and maps `serde_json::Error` into `Error::InvalidJson`.

The primary consumer is the EIDO crate: it deserializes a large JSON
document, extracts the raw jCard `Value` from the parent, and calls
`JCard::from_value()` to merge jCard warnings into the EIDO-level
`ParseWarning` list with proper field path prefixes.

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

## Empty parameter value lists and the param_values! macro

`ParamValue::Multiple(vec![])` is representable in Rust but semantically
invalid per RFC 7095 — it serializes to `"type": []`, which no consumer
should accept. The question is where to catch this.

Three approaches were considered:

**Passthrough** — `From<Vec<String>>` wraps whatever it gets. Simplest,
but the library silently produces non-conformant output. A caller who
accidentally passes an empty vec gets no signal; the bug surfaces far
downstream when a consumer rejects the jCard.

**TryFrom everywhere** — honest about fallibility, but removes the
ergonomic `.into()` path. Every call site becomes `.try_into()?`. Also,
`From` and `TryFrom` can't coexist for the same type pair due to the
blanket impl conflict.

**Compile-time macro + runtime TryFrom** — a `param_values!` macro
validates at compile time for literal usage, while `TryFrom<Vec<String>>`
catches empty vecs at runtime for programmatic callers.

```rust
param_values!["work", "voice"]  // → ParamValue::Multiple(vec![...])
param_values!["work"]           // → ParamValue::Single("work".into())
param_values![]                 // → compile_error!
```

The macro approach splits the problem along realistic lines. The static
builder path — where a developer writes `param_values!["work", "voice"]`
— gets compile-time rejection of empty lists and automatic `to_string()`
conversion. The single-element arm produces `Single` (the canonical form
per RFC 7095) rather than `Multiple(vec!["work"])`, avoiding a
non-canonical representation without any implicit runtime decision.

The dynamic path uses `TryFrom<Vec<String>>` which returns
`Err(EmptyParamValue)` on empty input. Callers building parameter lists
from runtime data get explicit error handling rather than silent
non-conformant output.

## Why property names are lowercased on construction

RFC 7095 §3.3 requires property names to be lowercase in the JSON
representation. Rather than validating at serialization time — which
would either silently produce non-conformant output or surprise the
caller with a runtime error far from where the property was created —
all constructors (`Property::new`, `Property::multi`, and the internal
`Property::from_raw`) normalize the name with `to_ascii_lowercase()`.

This means `Property::new("FN", ...)` produces a property with name
`"fn"`, matching what the RFC mandates on the wire. The normalization
is eager and visible: callers who inspect `property.name` immediately
see the lowercased form, so there are no surprises at serialization
time.
