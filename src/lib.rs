//! RFC 7095 jCard — JSON representation of vCard.
//!
//! Provides typed Rust structures for jCard documents with serde
//! serialization/deserialization matching the RFC 7095 JSON array format.
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
//! let jcard = JCard::builder()
//!     .fn_("Jane Doe")
//!     .n("Doe", "Jane", "", "", "")
//!     .email("jane.doe@example.com")
//!     .build();
//!
//! let json = serde_json::to_string(&jcard).unwrap();
//! let parsed: JCard = serde_json::from_str(&json).unwrap();
//! assert_eq!(jcard, parsed);
//! ```

mod deserialize;
pub mod property;
mod serialize;

use std::fmt;
use std::str::FromStr;

pub use property::{ParamValue, Property, PropertyValue, StructuredComponent};

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
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
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
    pub fn tel_with_type(self, uri: &str, types: Vec<String>) -> Self {
        self.property(
            Property::new("tel", PropertyValue::Uri(uri.to_string()))
                .with_param("type", ParamValue::from(types)),
        )
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
            jcard
                .get("version")
                .unwrap()
                .value,
            PropertyValue::Text("4.0".to_string())
        );
        assert_eq!(
            jcard
                .get("fn")
                .unwrap()
                .value,
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
            jcard
                .get("fn")
                .unwrap()
                .value,
            PropertyValue::Text("John Doe".to_string()),
        );
        assert_eq!(
            jcard
                .get("n")
                .unwrap()
                .value,
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
            bday.value,
            PropertyValue::DateAndOrTime("--02-03".to_string()),
        );

        let lang = jcard
            .get("lang")
            .unwrap();
        assert_eq!(lang.value_type, "language-tag");
        assert_eq!(lang.value, PropertyValue::LanguageTag("fr".to_string()),);

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
            .tel_with_type(
                "tel:+15555550100",
                vec!["work".to_string(), "voice".to_string()],
            )
            .build();

        let tel = jcard
            .get("tel")
            .unwrap();
        assert_eq!(
            tel.value,
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
            jcard
                .get("version")
                .unwrap()
                .value,
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
        assert_eq!(tz.value, PropertyValue::UtcOffset("-05:00".to_string()));

        let unknown = jcard
            .get("x-unknown-prop")
            .unwrap();
        assert_eq!(unknown.value_type, "unknown");
        assert_eq!(
            unknown.value,
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
            adr.value,
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
}
