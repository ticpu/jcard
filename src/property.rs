//! jCard property types per RFC 7095 §3.

use std::collections::BTreeMap;
use std::fmt;

/// A parameter value attached to a jCard property.
///
/// Per RFC 7095 §3.4, parameter values are either a single string or an
/// array of strings (for multi-valued parameters like `TYPE`).
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// A single parameter value.
    Single(String),
    /// Multiple parameter values (serialized as a JSON array).
    Multiple(Vec<String>),
}

impl From<&str> for ParamValue {
    fn from(s: &str) -> Self {
        Self::Single(s.to_string())
    }
}

impl From<String> for ParamValue {
    fn from(s: String) -> Self {
        Self::Single(s)
    }
}

impl From<Vec<String>> for ParamValue {
    fn from(v: Vec<String>) -> Self {
        if v.len() == 1 {
            Self::Single(
                v.into_iter()
                    .next()
                    .unwrap(),
            )
        } else {
            Self::Multiple(v)
        }
    }
}

impl fmt::Display for ParamValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(s) => write!(f, "{s}"),
            Self::Multiple(v) => write!(f, "{}", v.join(",")),
        }
    }
}

/// A component within a structured property value (RFC 7095 §3.3.1.3).
///
/// Structured values like `N` and `ADR` contain components separated by
/// semicolons in vCard. In jCard, these become array elements. Each
/// component is either a single value or a nested array when the component
/// itself has multiple values (e.g., multiple street lines in `ADR`).
#[derive(Debug, Clone, PartialEq)]
pub enum StructuredComponent {
    /// A single text component.
    Text(String),
    /// Multiple values for one component (nested array in JSON).
    Multi(Vec<String>),
}

impl fmt::Display for StructuredComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) => write!(f, "{s}"),
            Self::Multi(v) => write!(f, "{}", v.join(",")),
        }
    }
}

/// The value of a jCard property.
///
/// Covers all value types defined in RFC 7095 §3.5 plus structured values
/// from §3.3.1.3. The type identifier string (e.g. `"text"`, `"uri"`)
/// is stored separately on [`Property::value_type`] for round-trip fidelity.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Plain text value (`"text"`). RFC 7095 §3.5.1.
    Text(String),
    /// URI value (`"uri"`). RFC 7095 §3.5.2.
    Uri(String),
    /// Date value (`"date"`). RFC 7095 §3.5.3.
    Date(String),
    /// Time value (`"time"`). RFC 7095 §3.5.4.
    Time(String),
    /// Date-time value (`"date-time"`). RFC 7095 §3.5.5.
    DateTime(String),
    /// Date and/or time value (`"date-and-or-time"`). RFC 7095 §3.5.6.
    DateAndOrTime(String),
    /// Timestamp value (`"timestamp"`). RFC 7095 §3.5.7.
    Timestamp(String),
    /// Boolean value (`"boolean"`). RFC 7095 §3.5.8.
    Boolean(bool),
    /// Integer value (`"integer"`). RFC 7095 §3.5.9.
    Integer(i64),
    /// Floating-point value (`"float"`). RFC 7095 §3.5.10.
    Float(f64),
    /// UTC offset value (`"utc-offset"`). RFC 7095 §3.5.11.
    UtcOffset(String),
    /// Language tag value (`"language-tag"`). RFC 7095 §3.5.12.
    LanguageTag(String),
    /// Unknown value type for round-tripping (`"unknown"`). RFC 7095 §5.
    Unknown(String),
    /// Structured property value (`"text"` type). RFC 7095 §3.3.1.3.
    Structured(Vec<StructuredComponent>),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s)
            | Self::Uri(s)
            | Self::Date(s)
            | Self::Time(s)
            | Self::DateTime(s)
            | Self::DateAndOrTime(s)
            | Self::Timestamp(s)
            | Self::UtcOffset(s)
            | Self::LanguageTag(s)
            | Self::Unknown(s) => write!(f, "{s}"),
            Self::Structured(components) => {
                for (i, c) in components
                    .iter()
                    .enumerate()
                {
                    if i > 0 {
                        write!(f, ";")?;
                    }
                    write!(f, "{c}")?;
                }
                Ok(())
            }
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::Float(n) => write!(f, "{n}"),
        }
    }
}

impl PropertyValue {
    /// Returns the default RFC 7095 type identifier for this value variant.
    pub fn default_type(&self) -> &'static str {
        match self {
            Self::Text(_) | Self::Structured(_) => "text",
            Self::Uri(_) => "uri",
            Self::Date(_) => "date",
            Self::Time(_) => "time",
            Self::DateTime(_) => "date-time",
            Self::DateAndOrTime(_) => "date-and-or-time",
            Self::Timestamp(_) => "timestamp",
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::UtcOffset(_) => "utc-offset",
            Self::LanguageTag(_) => "language-tag",
            Self::Unknown(_) => "unknown",
        }
    }

    pub(crate) fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Text(s)
            | Self::Uri(s)
            | Self::Date(s)
            | Self::Time(s)
            | Self::DateTime(s)
            | Self::DateAndOrTime(s)
            | Self::Timestamp(s)
            | Self::UtcOffset(s)
            | Self::LanguageTag(s)
            | Self::Unknown(s) => serde_json::Value::String(s.clone()),
            Self::Structured(components) => serde_json::Value::Array(
                components
                    .iter()
                    .map(|c| match c {
                        StructuredComponent::Text(s) => serde_json::Value::String(s.clone()),
                        StructuredComponent::Multi(v) => serde_json::Value::Array(
                            v.iter()
                                .map(|s| serde_json::Value::String(s.clone()))
                                .collect(),
                        ),
                    })
                    .collect(),
            ),
            Self::Boolean(b) => serde_json::Value::Bool(*b),
            Self::Integer(i) => serde_json::json!(i),
            Self::Float(f) => serde_json::json!(f),
        }
    }

    pub(crate) fn from_json(value_type: &str, value: &serde_json::Value) -> Option<Self> {
        match value_type {
            "text" => match value {
                serde_json::Value::String(s) => Some(Self::Text(s.clone())),
                serde_json::Value::Array(arr) => Some(Self::Structured(parse_structured(arr))),
                _ => None,
            },
            "uri" => value
                .as_str()
                .map(|s| Self::Uri(s.to_string())),
            "date" => value
                .as_str()
                .map(|s| Self::Date(s.to_string())),
            "time" => value
                .as_str()
                .map(|s| Self::Time(s.to_string())),
            "date-time" => value
                .as_str()
                .map(|s| Self::DateTime(s.to_string())),
            "date-and-or-time" => value
                .as_str()
                .map(|s| Self::DateAndOrTime(s.to_string())),
            "timestamp" => value
                .as_str()
                .map(|s| Self::Timestamp(s.to_string())),
            "boolean" => value
                .as_bool()
                .map(Self::Boolean),
            "integer" => value
                .as_i64()
                .map(Self::Integer),
            "float" => value
                .as_f64()
                .map(Self::Float),
            "utc-offset" => value
                .as_str()
                .map(|s| Self::UtcOffset(s.to_string())),
            "language-tag" => value
                .as_str()
                .map(|s| Self::LanguageTag(s.to_string())),
            "unknown" => value
                .as_str()
                .map(|s| Self::Unknown(s.to_string())),
            _ => match value {
                serde_json::Value::String(s) => Some(Self::Text(s.clone())),
                serde_json::Value::Bool(b) => Some(Self::Boolean(*b)),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Some(Self::Integer(i))
                    } else {
                        n.as_f64()
                            .map(Self::Float)
                    }
                }
                _ => None,
            },
        }
    }
}

fn parse_structured(arr: &[serde_json::Value]) -> Vec<StructuredComponent> {
    arr.iter()
        .map(|v| match v {
            serde_json::Value::String(s) => StructuredComponent::Text(s.clone()),
            serde_json::Value::Array(inner) => StructuredComponent::Multi(
                inner
                    .iter()
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect(),
            ),
            other => StructuredComponent::Text(other.to_string()),
        })
        .collect()
}

/// A single jCard property.
///
/// Per RFC 7095 §3, a property is serialized as a JSON array tuple:
/// `[name, parameters, type, value1, value2, ...]`.
/// Most properties have a single value; multi-valued properties (§3.3)
/// like `CATEGORIES` carry additional values as extra array elements.
///
/// The [`value_type`](Self::value_type) field stores the RFC 7095 type
/// identifier exactly as received, ensuring round-trip fidelity even for
/// extension types not covered by [`PropertyValue`] variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Lowercase property name (e.g. `"fn"`, `"n"`, `"email"`).
    pub name: String,
    /// Property parameters as key-value pairs.
    pub parameters: BTreeMap<String, ParamValue>,
    /// The RFC 7095 type identifier (e.g. `"text"`, `"uri"`, `"date-time"`).
    pub value_type: String,
    values: Vec<PropertyValue>,
}

impl Property {
    /// Creates a single-valued property.
    ///
    /// The type identifier is derived from the [`PropertyValue`] variant.
    pub fn new(name: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            name: name.into(),
            parameters: BTreeMap::new(),
            value_type: value
                .default_type()
                .to_string(),
            values: vec![value],
        }
    }

    /// Creates a multi-valued property (RFC 7095 §3.3).
    ///
    /// Panics if `values` is empty.
    pub fn multi(name: impl Into<String>, values: Vec<PropertyValue>) -> Self {
        assert!(!values.is_empty(), "property must have at least one value");
        Self {
            name: name.into(),
            parameters: BTreeMap::new(),
            value_type: values[0]
                .default_type()
                .to_string(),
            values,
        }
    }

    pub(crate) fn from_raw(
        name: String,
        parameters: BTreeMap<String, ParamValue>,
        value_type: String,
        values: Vec<PropertyValue>,
    ) -> Self {
        Self {
            name,
            parameters,
            value_type,
            values,
        }
    }

    /// Returns the first (or only) value.
    pub fn value(&self) -> &PropertyValue {
        &self.values[0]
    }

    /// Returns all values. Single-valued properties return a one-element slice.
    /// Multi-valued properties (RFC 7095 §3.3) return multiple elements.
    pub fn values(&self) -> &[PropertyValue] {
        &self.values
    }

    /// Adds a parameter to this property (builder pattern).
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<ParamValue>) -> Self {
        self.parameters
            .insert(key.into(), value.into());
        self
    }
}
