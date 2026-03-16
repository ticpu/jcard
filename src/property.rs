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

/// The value of a jCard property.
///
/// Per RFC 7095 §3.3, each property has a typed value. The type identifier
/// string is derived from the variant (e.g. `Text` → `"text"`).
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Plain text value (`"text"` type identifier).
    Text(String),
    /// Structured text with components (`"text"` type, serialized as JSON array).
    /// Used for properties like `N` with multiple name parts.
    TextList(Vec<String>),
    /// URI value (`"uri"` type identifier).
    Uri(String),
    /// Boolean value (`"boolean"` type identifier).
    Boolean(bool),
    /// Integer value (`"integer"` type identifier).
    Integer(i64),
    /// Floating-point value (`"float"` type identifier).
    Float(f64),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) | Self::Uri(s) => write!(f, "{s}"),
            Self::TextList(v) => write!(f, "{}", v.join(";")),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::Float(n) => write!(f, "{n}"),
        }
    }
}

impl PropertyValue {
    /// Returns the RFC 7095 type identifier string for this value.
    pub fn value_type(&self) -> &'static str {
        match self {
            Self::Text(_) | Self::TextList(_) => "text",
            Self::Uri(_) => "uri",
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
        }
    }

    pub(crate) fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Text(s) => serde_json::Value::String(s.clone()),
            Self::TextList(v) => serde_json::Value::Array(
                v.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
            Self::Uri(s) => serde_json::Value::String(s.clone()),
            Self::Boolean(b) => serde_json::Value::Bool(*b),
            Self::Integer(i) => serde_json::json!(i),
            Self::Float(f) => serde_json::json!(f),
        }
    }

    pub(crate) fn from_json(value_type: &str, values: &[serde_json::Value]) -> Option<Self> {
        let first = values.first()?;
        match value_type {
            "text" => match first {
                serde_json::Value::String(s) => Some(Self::Text(s.clone())),
                serde_json::Value::Array(arr) => {
                    let parts: Vec<String> = arr
                        .iter()
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    Some(Self::TextList(parts))
                }
                _ => None,
            },
            "uri" => first
                .as_str()
                .map(|s| Self::Uri(s.to_string())),
            "boolean" => first
                .as_bool()
                .map(Self::Boolean),
            "integer" => first
                .as_i64()
                .map(Self::Integer),
            "float" => first
                .as_f64()
                .map(Self::Float),
            _ => first
                .as_str()
                .map(|s| Self::Text(s.to_string())),
        }
    }
}

/// A single jCard property.
///
/// Per RFC 7095 §3, a property is serialized as a JSON array tuple:
/// `[name, parameters, type, value]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Lowercase property name (e.g. `"fn"`, `"n"`, `"email"`).
    pub name: String,
    /// Property parameters as key-value pairs.
    pub parameters: BTreeMap<String, ParamValue>,
    /// The typed property value.
    pub value: PropertyValue,
}

impl Property {
    /// Creates a new property with the given name and value, and no parameters.
    pub fn new(name: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            name: name.into(),
            parameters: BTreeMap::new(),
            value,
        }
    }

    /// Adds a parameter to this property (builder pattern).
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<ParamValue>) -> Self {
        self.parameters
            .insert(key.into(), value.into());
        self
    }

    /// Returns the RFC 7095 type identifier for this property's value.
    pub fn value_type(&self) -> &str {
        self.value
            .value_type()
    }
}
