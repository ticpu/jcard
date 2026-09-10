//! RFC 7095 jCard — JSON representation of vCard.
//!
//! Provides typed Rust structures for jCard documents with serde
//! serialization/deserialization matching the RFC 7095 JSON array format.
//!
//! # Lenient Parsing
//!
//! [`JCard::from_json`] returns [`Parsed<JCard>`] — the best-effort parse
//! result plus any [`ParseWarning`] entries for malformed properties.
//! This follows the EIDO pattern for NG9-1-1: calltakers see all
//! available data while discrepancy reports capture what went wrong.
//!
//! The serde [`Deserialize`](serde::Deserialize) impl is also available
//! for simple use cases where warnings are not needed. By default it
//! returns `Err` on structural failures; enable the `lenient-deserialize`
//! feature to return an empty `JCard` instead (useful when embedding
//! jCard in a parent struct that must not fail on malformed contact data).
//!
//! # Value Types
//!
//! All RFC 7095 §3.5 value types are supported as [`PropertyValue`] variants:
//! text, uri, date, time, date-time, date-and-or-time, timestamp,
//! boolean, integer, float, utc-offset, language-tag, and unknown.
//! Structured property values (e.g. `N`, `ADR`) use
//! [`PropertyValue::Structured`] with [`StructuredComponent`] elements,
//! including nested arrays per §3.3.1.3.
//!
//! The type identifier is stored on [`Property::value_type`] separately
//! from the value, ensuring round-trip fidelity for all types including
//! extensions.
//!
//! # Examples
//!
//! ```
//! use jcard::JCard;
//!
//! // Lenient parsing with warnings
//! let parsed = JCard::from_json(r#"["vcard",[["version",{},"text","4.0"]]]"#).unwrap();
//! assert!(!parsed.has_warnings());
//!
//! // Simple serde path (warnings discarded)
//! let jcard: JCard = serde_json::from_str(r#"["vcard",[["version",{},"text","4.0"]]]"#).unwrap();
//! ```

mod deserialize;
pub mod error;
pub mod property;
mod serialize;
#[cfg(feature = "xcard")]
pub mod xcard;

use std::fmt;
use std::str::FromStr;

pub use error::Error;
pub use property::{EmptyParamValue, ParamValue, Property, PropertyValue, StructuredComponent};

/// Whether the value survived the problem a [`ParseWarning`] reports.
///
/// Both are worth reporting upstream, but only one of them costs the reader
/// data, so a consumer that acts on warnings needs to tell them apart without
/// matching on the message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    /// The input was non-conformant and the value came through anyway, in a
    /// form that round-trips.
    Recovered,
    /// The value is gone: a property, parameter, value or component was
    /// dropped, or was never there.
    Lost,
}

/// Warning emitted during lenient jCard parsing.
///
/// When a property can't be fully parsed, the parser preserves what it can
/// and emits a warning describing what went wrong. The raw unparsed value
/// is available in [`raw_value`](Self::raw_value) when applicable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWarning {
    /// Path to the problematic element (e.g. `properties[3]`).
    pub path: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// Unparsed source text, preserved for display.
    pub raw_value: Option<String>,
    /// Whether the value survived (see [`WarningKind`]).
    pub kind: WarningKind,
}

impl ParseWarning {
    /// Warning whose value is gone.
    pub fn lost(
        path: impl Into<String>,
        message: impl Into<String>,
        raw_value: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            raw_value,
            kind: WarningKind::Lost,
        }
    }

    /// Warning whose value came through the non-conformance intact.
    pub fn recovered(
        path: impl Into<String>,
        message: impl Into<String>,
        raw_value: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            raw_value,
            kind: WarningKind::Recovered,
        }
    }
}

impl fmt::Display for ParseWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.raw_value {
            Some(raw) => write!(f, "{}: {} (raw: {raw})", self.path, self.message),
            None => write!(f, "{}: {}", self.path, self.message),
        }
    }
}

/// Result of lenient jCard parsing: the parsed value plus any warnings.
///
/// Even when `warnings` is non-empty, `value` contains everything that
/// could be successfully parsed.
#[derive(Debug, Clone)]
pub struct Parsed<T> {
    /// Best-effort parse result.
    pub value: T,
    /// Problems encountered during parsing.
    pub warnings: Vec<ParseWarning>,
}

impl<T> Parsed<T> {
    /// Returns `true` if any warnings were collected during parsing.
    pub fn has_warnings(&self) -> bool {
        !self
            .warnings
            .is_empty()
    }
}

/// A jCard document (RFC 7095).
///
/// Serializes to `["vcard", [...properties]]` per the RFC 7095 format.
/// A new `JCard` always includes the mandatory `version` property set to `"4.0"`.
#[derive(Debug, Clone, PartialEq)]
pub struct JCard {
    properties: Vec<Property>,
}

impl JCard {
    /// Creates a new jCard with only the mandatory `version` property.
    pub fn new() -> Self {
        Self {
            properties: vec![Property::new(
                "version",
                PropertyValue::Text("4.0".to_string()),
            )],
        }
    }

    /// Returns a builder for ergonomic jCard construction.
    pub fn builder() -> JCardBuilder {
        JCardBuilder { jcard: Self::new() }
    }

    /// Returns the properties in this jCard.
    pub fn properties(&self) -> &[Property] {
        &self.properties
    }

    /// Returns a mutable reference to the properties list.
    pub fn properties_mut(&mut self) -> &mut Vec<Property> {
        &mut self.properties
    }

    /// Appends a property to this jCard.
    pub fn push(&mut self, property: Property) {
        self.properties
            .push(property);
    }

    /// Returns the first property with the given name, or `None`.
    pub fn get(&self, name: &str) -> Option<&Property> {
        self.properties
            .iter()
            .find(|p| p.name == name)
    }

    /// Returns all properties with the given name.
    pub fn get_all(&self, name: &str) -> Vec<&Property> {
        self.properties
            .iter()
            .filter(|p| p.name == name)
            .collect()
    }

    /// Returns the text of the first property with the given name.
    ///
    /// `None` when the property is absent or its value carries no text.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.get(name)?
            .as_str()
    }

    /// Returns the text of every property with the given name, in document
    /// order. Instances whose value carries no text are omitted.
    pub fn texts(&self, name: &str) -> Vec<&str> {
        self.get_all(name)
            .into_iter()
            .filter_map(Property::as_str)
            .collect()
    }

    /// Returns the components of the first property with the given name.
    ///
    /// `None` when the property is absent or its value is not structured.
    pub fn components(&self, name: &str) -> Option<&[StructuredComponent]> {
        self.get(name)?
            .components()
    }

    /// Returns the `FN` (formatted name) property's text.
    pub fn fn_(&self) -> Option<&str> {
        self.text("fn")
    }

    /// Returns the components of the `N` (structured name) property, family
    /// name first per RFC 6350 §6.2.2.
    pub fn n(&self) -> Option<&[StructuredComponent]> {
        self.components("n")
    }

    /// Returns the `ORG` property's text.
    pub fn org(&self) -> Option<&str> {
        self.text("org")
    }

    /// Returns the `TITLE` property's text.
    pub fn title(&self) -> Option<&str> {
        self.text("title")
    }

    /// Returns the `BDAY` (birthday) property's text.
    pub fn bday(&self) -> Option<&str> {
        self.text("bday")
    }

    /// Returns the `REV` (revision) property's text.
    pub fn rev(&self) -> Option<&str> {
        self.text("rev")
    }

    /// Returns the text of every `EMAIL` property.
    pub fn emails(&self) -> Vec<&str> {
        self.texts("email")
    }

    /// Returns the text of every `TEL` property.
    pub fn tels(&self) -> Vec<&str> {
        self.texts("tel")
    }

    /// Returns the text of every `URL` property.
    pub fn urls(&self) -> Vec<&str> {
        self.texts("url")
    }

    /// Returns the text of every `NOTE` property.
    pub fn notes(&self) -> Vec<&str> {
        self.texts("note")
    }

    /// Returns the text of every `LANG` property.
    pub fn langs(&self) -> Vec<&str> {
        self.texts("lang")
    }

    /// Returns the components of every `ADR` property, per RFC 6350 §6.3.1.
    pub fn adrs(&self) -> Vec<&[StructuredComponent]> {
        self.get_all("adr")
            .into_iter()
            .filter_map(Property::components)
            .collect()
    }

    /// Parses a jCard from a [`serde_json::Value`] with lenient error handling.
    ///
    /// Returns [`Parsed<JCard>`] containing the best-effort parse result
    /// and any [`ParseWarning`] entries for malformed properties.
    /// Returns [`Error`] only for structural failures (missing `"vcard"`
    /// tag, wrong element count, etc.).
    ///
    /// Use this when you already have a parsed JSON value (e.g., extracted
    /// from a parent document). For raw JSON strings, use [`from_json`](Self::from_json).
    pub fn from_value(value: &serde_json::Value) -> Result<Parsed<JCard>, Error> {
        let mut warnings = Vec::new();
        let jcard = deserialize::parse_jcard_value(value, &mut warnings)?;
        Ok(Parsed {
            value: jcard,
            warnings,
        })
    }

    /// Parses a jCard from a JSON string with lenient error handling.
    ///
    /// Returns [`Parsed<JCard>`] containing the best-effort parse result
    /// and any [`ParseWarning`] entries for malformed properties.
    /// Returns [`Error`] only for structural failures (invalid JSON or
    /// not a jCard at all).
    pub fn from_json(json: &str) -> Result<Parsed<JCard>, Error> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| Error::InvalidJson(Box::new(e)))?;
        Self::from_value(&value)
    }
}

impl Default for JCard {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_string(self) {
            Ok(s) => f.write_str(&s),
            Err(_) => Err(fmt::Error),
        }
    }
}

impl FromStr for JCard {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_json(s)?.value)
    }
}

/// Builder for constructing jCard documents with common properties.
pub struct JCardBuilder {
    jcard: JCard,
}

impl JCardBuilder {
    /// Adds an arbitrary property.
    pub fn property(mut self, prop: Property) -> Self {
        self.jcard
            .push(prop);
        self
    }

    /// Adds an `FN` (formatted name) property.
    pub fn fn_(self, name: &str) -> Self {
        self.property(Property::new("fn", PropertyValue::Text(name.to_string())))
    }

    /// Adds an `N` (structured name) property with five components per RFC 6350 §6.2.2.
    pub fn n(
        self,
        family: &str,
        given: &str,
        additional: &str,
        prefix: &str,
        suffix: &str,
    ) -> Self {
        self.property(Property::new(
            "n",
            PropertyValue::Structured(vec![
                StructuredComponent::Text(family.to_string()),
                StructuredComponent::Text(given.to_string()),
                StructuredComponent::Text(additional.to_string()),
                StructuredComponent::Text(prefix.to_string()),
                StructuredComponent::Text(suffix.to_string()),
            ]),
        ))
    }

    /// Adds an `EMAIL` property.
    pub fn email(self, email: &str) -> Self {
        self.property(Property::new(
            "email",
            PropertyValue::Text(email.to_string()),
        ))
    }

    /// Adds an `EMAIL` property with a `TYPE` parameter.
    pub fn email_with_type(self, email: &str, type_: &str) -> Self {
        self.property(
            Property::new("email", PropertyValue::Text(email.to_string()))
                .with_param("type", type_),
        )
    }

    /// Adds a `TEL` property with a URI value.
    pub fn tel(self, uri: &str) -> Self {
        self.property(Property::new("tel", PropertyValue::Uri(uri.to_string())))
    }

    /// Adds a `TEL` property with a URI value and `TYPE` parameters.
    ///
    /// If `types` is empty, the property is added without a `TYPE` parameter.
    pub fn tel_with_type(self, uri: &str, types: &[&str]) -> Self {
        let prop = Property::new("tel", PropertyValue::Uri(uri.to_string()));
        if types.is_empty() {
            return self.property(prop);
        }
        let param = if types.len() == 1 {
            ParamValue::Single(types[0].to_string())
        } else {
            ParamValue::Multiple(
                types
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            )
        };
        self.property(prop.with_param("type", param))
    }

    /// Adds an `ORG` property.
    pub fn org(self, org: &str) -> Self {
        self.property(Property::new("org", PropertyValue::Text(org.to_string())))
    }

    /// Adds a `TITLE` property.
    pub fn title(self, title: &str) -> Self {
        self.property(Property::new(
            "title",
            PropertyValue::Text(title.to_string()),
        ))
    }

    /// Adds an `ADR` (structured address) property per RFC 6350 §6.3.1.
    pub fn adr(self, components: Vec<StructuredComponent>) -> Self {
        self.property(Property::new("adr", PropertyValue::Structured(components)))
    }

    /// Adds a `BDAY` (birthday) property with a date-and-or-time value.
    pub fn bday(self, value: &str) -> Self {
        self.property(Property::new(
            "bday",
            PropertyValue::DateAndOrTime(value.to_string()),
        ))
    }

    /// Adds a `URL` property.
    pub fn url(self, uri: &str) -> Self {
        self.property(Property::new("url", PropertyValue::Uri(uri.to_string())))
    }

    /// Adds a `NOTE` property.
    pub fn note(self, text: &str) -> Self {
        self.property(Property::new("note", PropertyValue::Text(text.to_string())))
    }

    /// Adds a `REV` (revision) property with a timestamp value.
    pub fn rev(self, timestamp: &str) -> Self {
        self.property(Property::new(
            "rev",
            PropertyValue::Timestamp(timestamp.to_string()),
        ))
    }

    /// Consumes the builder and returns the constructed [`JCard`].
    pub fn build(self) -> JCard {
        self.jcard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_produces_valid_jcard() {
        let jcard = JCard::builder()
            .fn_("Jane Doe")
            .n("Doe", "Jane", "", "", "")
            .email("jane.doe@example.com")
            .build();

        assert_eq!(
            jcard
                .properties()
                .len(),
            4
        );
        assert_eq!(
            *jcard
                .get("version")
                .unwrap()
                .value(),
            PropertyValue::Text("4.0".to_string())
        );
        assert_eq!(
            *jcard
                .get("fn")
                .unwrap()
                .value(),
            PropertyValue::Text("Jane Doe".to_string())
        );
    }

    #[test]
    fn serialize_roundtrip() {
        let jcard = JCard::builder()
            .fn_("Jane Doe")
            .n("Doe", "Jane", "", "", "")
            .email("jane.doe@example.com")
            .build();

        let json = serde_json::to_string(&jcard).unwrap();
        let parsed: JCard = serde_json::from_str(&json).unwrap();
        assert_eq!(jcard, parsed);
    }

    #[test]
    fn serialize_format_matches_rfc7095() {
        let jcard = JCard::builder()
            .fn_("Test User")
            .build();

        let json = serde_json::to_string(&jcard).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value[0], "vcard");
        assert!(value[1].is_array());

        let version = &value[1][0];
        assert_eq!(version[0], "version");
        assert_eq!(version[1], serde_json::json!({}));
        assert_eq!(version[2], "text");
        assert_eq!(version[3], "4.0");

        let fn_prop = &value[1][1];
        assert_eq!(fn_prop[0], "fn");
        assert_eq!(fn_prop[2], "text");
        assert_eq!(fn_prop[3], "Test User");
    }

    /// RFC 7095 Appendix B example with synthetic test data.
    #[test]
    fn deserialize_rfc7095_example() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text","John Doe"],
            ["n",{},"text",["Doe","John","","",""]],
            ["bday",{},"date-and-or-time","--02-03"],
            ["gender",{},"text","M"],
            ["lang",{"pref":"1"},"language-tag","fr"],
            ["tel",{"type":["work","voice"],"pref":"1"},"uri","tel:+15551234567;ext=102"],
            ["email",{"type":"work"},"text","john.doe@example.com"]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        assert_eq!(
            jcard
                .properties()
                .len(),
            8
        );
        assert_eq!(
            *jcard
                .get("fn")
                .unwrap()
                .value(),
            PropertyValue::Text("John Doe".to_string()),
        );
        assert_eq!(
            *jcard
                .get("n")
                .unwrap()
                .value(),
            PropertyValue::Structured(vec![
                StructuredComponent::Text("Doe".to_string()),
                StructuredComponent::Text("John".to_string()),
                StructuredComponent::Text(String::new()),
                StructuredComponent::Text(String::new()),
                StructuredComponent::Text(String::new()),
            ]),
        );

        let bday = jcard
            .get("bday")
            .unwrap();
        assert_eq!(bday.value_type, "date-and-or-time");
        assert_eq!(
            *bday.value(),
            PropertyValue::DateAndOrTime("--02-03".to_string()),
        );

        let lang = jcard
            .get("lang")
            .unwrap();
        assert_eq!(lang.value_type, "language-tag");
        assert_eq!(*lang.value(), PropertyValue::LanguageTag("fr".to_string()),);

        let tel = jcard
            .get("tel")
            .unwrap();
        assert_eq!(
            tel.parameters
                .get("type"),
            Some(&ParamValue::Multiple(vec![
                "work".to_string(),
                "voice".to_string(),
            ])),
        );
    }

    #[test]
    fn display_and_fromstr() {
        let jcard = JCard::builder()
            .fn_("Test")
            .build();

        let display = jcard.to_string();
        let parsed: JCard = display
            .parse()
            .unwrap();
        assert_eq!(jcard, parsed);
    }

    #[test]
    fn tel_with_type_params() {
        let jcard = JCard::builder()
            .tel_with_type("tel:+15555550100", &["work", "voice"])
            .build();

        let tel = jcard
            .get("tel")
            .unwrap();
        assert_eq!(
            *tel.value(),
            PropertyValue::Uri("tel:+15555550100".to_string())
        );
        assert!(tel
            .parameters
            .contains_key("type"));
    }

    #[test]
    fn empty_jcard_has_version() {
        let jcard = JCard::new();
        assert_eq!(
            jcard
                .properties()
                .len(),
            1
        );
        assert_eq!(
            *jcard
                .get("version")
                .unwrap()
                .value(),
            PropertyValue::Text("4.0".to_string())
        );
    }

    #[test]
    fn get_all_returns_multiple() {
        let jcard = JCard::builder()
            .email("a@example.com")
            .email("b@example.com")
            .build();

        assert_eq!(
            jcard
                .get_all("email")
                .len(),
            2
        );
    }

    #[test]
    fn all_value_types_roundtrip() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text","Test"],
            ["bday",{},"date","1985-04-12"],
            ["x-time-example",{},"time","12:30:00"],
            ["anniversary",{},"date-time","2013-02-14T12:30:00"],
            ["bday",{},"date-and-or-time","--02-03"],
            ["rev",{},"timestamp","2013-02-14T12:30:00Z"],
            ["x-non-smoking",{},"boolean",true],
            ["x-karma-points",{},"integer",42],
            ["x-grade",{},"float",1.3],
            ["tz",{},"utc-offset","-05:00"],
            ["lang",{"pref":"1"},"language-tag","fr"],
            ["x-unknown-prop",{},"unknown","some;raw\\,data"]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        let reserialized = serde_json::to_string(&jcard).unwrap();
        let reparsed: JCard = serde_json::from_str(&reserialized).unwrap();
        assert_eq!(jcard, reparsed);

        let bday_props = jcard.get_all("bday");
        assert_eq!(bday_props[0].value_type, "date");
        assert_eq!(bday_props[1].value_type, "date-and-or-time");

        let tz = jcard
            .get("tz")
            .unwrap();
        assert_eq!(tz.value_type, "utc-offset");
        assert_eq!(*tz.value(), PropertyValue::UtcOffset("-05:00".to_string()));

        let unknown = jcard
            .get("x-unknown-prop")
            .unwrap();
        assert_eq!(unknown.value_type, "unknown");
        assert_eq!(
            *unknown.value(),
            PropertyValue::Unknown("some;raw\\,data".to_string()),
        );
    }

    #[test]
    fn structured_value_with_nested_arrays() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["adr",{},"text",
                ["","",["123 Main St","Suite 100"],"Any Town","CA","91921","U.S.A."]
            ]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        let adr = jcard
            .get("adr")
            .unwrap();
        assert_eq!(
            *adr.value(),
            PropertyValue::Structured(vec![
                StructuredComponent::Text(String::new()),
                StructuredComponent::Text(String::new()),
                StructuredComponent::Multi(vec![
                    "123 Main St".to_string(),
                    "Suite 100".to_string(),
                ]),
                StructuredComponent::Text("Any Town".to_string()),
                StructuredComponent::Text("CA".to_string()),
                StructuredComponent::Text("91921".to_string()),
                StructuredComponent::Text("U.S.A.".to_string()),
            ]),
        );

        let json_out = serde_json::to_string(&jcard).unwrap();
        let reparsed: JCard = serde_json::from_str(&json_out).unwrap();
        assert_eq!(jcard, reparsed);
    }

    #[test]
    fn group_parameter() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{"group":"contact"},"text","Mr. John Q. Public, Esq."]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        let fn_prop = jcard
            .get("fn")
            .unwrap();
        assert_eq!(
            fn_prop
                .parameters
                .get("group"),
            Some(&ParamValue::Single("contact".to_string())),
        );

        let json_out = serde_json::to_string(&jcard).unwrap();
        let reparsed: JCard = serde_json::from_str(&json_out).unwrap();
        assert_eq!(jcard, reparsed);
    }

    #[test]
    fn extension_type_preserved() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["x-custom",{},"x-mytype","custom-value"]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        let custom = jcard
            .get("x-custom")
            .unwrap();
        assert_eq!(custom.value_type, "x-mytype");

        let json_out = serde_json::to_string(&jcard).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&json_out).unwrap();
        assert_eq!(reparsed[1][1][2], "x-mytype");
    }

    #[test]
    fn multi_valued_property_roundtrip() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["categories",{},"text","computers","cameras"]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        let cat = jcard
            .get("categories")
            .unwrap();
        assert_eq!(
            cat.values(),
            &[
                PropertyValue::Text("computers".to_string()),
                PropertyValue::Text("cameras".to_string()),
            ]
        );
        assert_eq!(*cat.value(), PropertyValue::Text("computers".to_string()),);

        let json_out = serde_json::to_string(&jcard).unwrap();
        let reparsed: JCard = serde_json::from_str(&json_out).unwrap();
        assert_eq!(jcard, reparsed);

        let raw: serde_json::Value = serde_json::from_str(&json_out).unwrap();
        assert_eq!(raw[1][1][3], "computers");
        assert_eq!(raw[1][1][4], "cameras");
    }

    #[test]
    fn multi_valued_via_builder() {
        let jcard = JCard::builder()
            .property(
                Property::multi(
                    "categories",
                    vec![
                        PropertyValue::Text("computers".to_string()),
                        PropertyValue::Text("cameras".to_string()),
                    ],
                )
                .unwrap(),
            )
            .build();

        let cat = jcard
            .get("categories")
            .unwrap();
        assert_eq!(
            cat.values()
                .len(),
            2
        );

        let json = serde_json::to_string(&jcard).unwrap();
        let parsed: JCard = serde_json::from_str(&json).unwrap();
        assert_eq!(jcard, parsed);
    }

    #[test]
    fn from_json_clean_input_no_warnings() {
        let parsed =
            JCard::from_json(r#"["vcard",[["version",{},"text","4.0"],["fn",{},"text","Test"]]]"#)
                .unwrap();
        assert!(!parsed.has_warnings());
        assert_eq!(
            parsed
                .value
                .properties()
                .len(),
            2
        );
    }

    #[test]
    fn from_json_short_tuple_warns_and_drops() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text"],
            ["email",{},"text","ok@example.com"]
        ]]"#;

        let parsed = JCard::from_json(json).unwrap();
        assert_eq!(
            parsed
                .warnings
                .len(),
            1
        );
        assert!(parsed.warnings[0]
            .message
            .contains("3 elements"));
        assert_eq!(
            parsed
                .value
                .properties()
                .len(),
            2
        );
    }

    #[test]
    fn from_json_value_type_mismatch_warns_and_preserves() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text",42]
        ]]"#;

        let parsed = JCard::from_json(json).unwrap();
        assert_eq!(
            parsed
                .warnings
                .len(),
            1
        );
        assert!(parsed.warnings[0]
            .message
            .contains("expected text"));
        assert_eq!(
            parsed
                .value
                .properties()
                .len(),
            2
        );
        assert_eq!(
            *parsed
                .value
                .get("fn")
                .unwrap()
                .value(),
            PropertyValue::Text("42".to_string()),
        );
    }

    #[test]
    fn from_json_bad_params_warns_and_preserves() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn","not-an-object","text","Test"]
        ]]"#;

        let parsed = JCard::from_json(json).unwrap();
        assert_eq!(
            parsed
                .warnings
                .len(),
            1
        );
        assert!(parsed.warnings[0]
            .message
            .contains("parameters is not an object"));
        let fn_prop = parsed
            .value
            .get("fn")
            .unwrap();
        assert!(fn_prop
            .parameters
            .is_empty());
        assert_eq!(*fn_prop.value(), PropertyValue::Text("Test".to_string()));
    }

    #[test]
    fn from_json_wrong_tag_is_error() {
        let result = JCard::from_json(r#"["vcalendar",[]]"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidStructure(_)));
        assert!(err
            .to_string()
            .contains("vcalendar"));
    }

    #[test]
    fn from_json_invalid_json_is_error() {
        let result = JCard::from_json("{not json}");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidJson(_)));
    }

    #[test]
    fn from_json_missing_version_warns() {
        let json = r#"["vcard",[["fn",{},"text","Test"]]]"#;
        let parsed = JCard::from_json(json).unwrap();
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w
                .message
                .contains("version")));
    }

    #[test]
    fn property_name_lowercased() {
        let prop = Property::new("FN", PropertyValue::Text("Test".to_string()));
        assert_eq!(prop.name, "fn");

        let prop =
            Property::multi("CATEGORIES", vec![PropertyValue::Text("a".to_string())]).unwrap();
        assert_eq!(prop.name, "categories");
    }

    #[test]
    fn from_value_matches_from_json() {
        let json = r#"["vcard",[["version",{},"text","4.0"],["fn",{},"text","Test"]]]"#;
        let from_json = JCard::from_json(json).unwrap();

        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let from_value = JCard::from_value(&value).unwrap();

        assert_eq!(from_json.value, from_value.value);
        assert_eq!(
            from_json
                .warnings
                .len(),
            from_value
                .warnings
                .len()
        );
    }

    #[test]
    #[cfg(feature = "lenient-deserialize")]
    fn deserialize_returns_default_on_structural_error() {
        let bad_tag: JCard = serde_json::from_str(r#"["vcalendar",[]]"#).unwrap();
        assert_eq!(bad_tag, JCard::default());

        let not_array: JCard = serde_json::from_str(r#""just a string""#).unwrap();
        assert_eq!(not_array, JCard::default());
    }

    #[test]
    #[cfg(not(feature = "lenient-deserialize"))]
    fn deserialize_returns_err_on_structural_error() {
        let result = serde_json::from_str::<JCard>(r#"["vcalendar",[]]"#);
        assert!(result.is_err());

        let result = serde_json::from_str::<JCard>(r#""just a string""#);
        assert!(result.is_err());
    }

    #[test]
    fn tel_with_empty_types_omits_param() {
        let jcard = JCard::builder()
            .tel_with_type("tel:+15555550100", &[])
            .build();

        let tel = jcard
            .get("tel")
            .unwrap();
        assert!(tel
            .parameters
            .is_empty());
    }

    #[test]
    fn multi_returns_none_on_empty() {
        assert!(Property::multi("categories", vec![]).is_none());
    }

    #[test]
    fn from_json_rfc_appendix_b_no_warnings() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text","John Doe"],
            ["n",{},"text",["Doe","John","","",""]],
            ["bday",{},"date-and-or-time","--02-03"],
            ["gender",{},"text","M"],
            ["lang",{"pref":"1"},"language-tag","fr"],
            ["tel",{"type":["work","voice"],"pref":"1"},"uri","tel:+15551234567;ext=102"],
            ["email",{"type":"work"},"text","john.doe@example.com"]
        ]]"#;

        let parsed = JCard::from_json(json).unwrap();
        assert!(
            !parsed.has_warnings(),
            "unexpected warnings: {:?}",
            parsed.warnings
        );
    }

    #[test]
    fn typed_accessors_read_appendix_b() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text","John Doe"],
            ["n",{},"text",["Doe","John","","",""]],
            ["bday",{},"date-and-or-time","--02-03"],
            ["lang",{"pref":"1"},"language-tag","fr"],
            ["tel",{"type":["work","voice"],"pref":"1"},"uri","tel:+15551234567;ext=102"],
            ["email",{"type":"work"},"text","john.doe@example.com"],
            ["email",{},"text","jdoe@example.org"]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        assert_eq!(jcard.fn_(), Some("John Doe"));
        assert_eq!(jcard.bday(), Some("--02-03"));
        assert_eq!(jcard.tels(), vec!["tel:+15551234567;ext=102"]);
        assert_eq!(jcard.langs(), vec!["fr"]);
        assert_eq!(
            jcard.emails(),
            vec!["john.doe@example.com", "jdoe@example.org"]
        );
        assert_eq!(jcard.org(), None);

        let n = jcard
            .n()
            .unwrap();
        assert_eq!(n[0].as_str(), Some("Doe"));
        assert_eq!(n[1].as_str(), Some("John"));
    }

    #[test]
    fn as_str_none_for_non_text_values() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["n",{},"text",["Doe","John","","",""]],
            ["x-non-smoking",{},"boolean",true],
            ["x-karma-points",{},"integer",42]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        assert_eq!(jcard.text("n"), None);
        assert_eq!(jcard.text("x-non-smoking"), None);
        assert_eq!(jcard.text("x-karma-points"), None);
        assert!(jcard
            .texts("n")
            .is_empty());
    }

    #[test]
    fn pref_parameter_read_not_applied() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["fn",{},"text","J. Doe"],
            ["fn",{"pref":"1"},"text","John Doe"]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        assert_eq!(jcard.fn_(), Some("J. Doe"));

        let all = jcard.get_all("fn");
        assert_eq!(all[0].pref(), None);
        assert_eq!(all[1].pref(), Some(1));
    }

    #[test]
    fn adr_components_expose_multi() {
        let json = r#"["vcard",[
            ["version",{},"text","4.0"],
            ["adr",{},"text",
                ["","",["123 Main St","Suite 100"],"Any Town","CA","91921","U.S.A."]
            ]
        ]]"#;

        let jcard: JCard = serde_json::from_str(json).unwrap();
        let adrs = jcard.adrs();
        assert_eq!(adrs.len(), 1);
        assert_eq!(adrs[0][2].as_str(), None);
        assert_eq!(adrs[0][3].as_str(), Some("Any Town"));
    }
}
