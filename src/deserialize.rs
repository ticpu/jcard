use std::collections::BTreeMap;

use serde::de::{Deserialize, Deserializer};

use crate::error::Error;
use crate::property::{ParamValue, Property, PropertyValue};
use crate::{JCard, ParseWarning};

/// Parses a jCard from a JSON value, collecting warnings for malformed
/// properties instead of dropping them silently.
pub(crate) fn parse_jcard_value(
    value: &serde_json::Value,
    warnings: &mut Vec<ParseWarning>,
) -> Result<JCard, Error> {
    let arr = value
        .as_array()
        .ok_or_else(|| Error::InvalidStructure("expected a JSON array".into()))?;

    if arr.len() < 2 {
        return Err(Error::InvalidStructure(
            "expected [\"vcard\", [...properties]]".into(),
        ));
    }

    let tag = arr[0]
        .as_str()
        .ok_or_else(|| {
            Error::InvalidStructure("first element must be the string \"vcard\"".into())
        })?;

    if tag != "vcard" {
        return Err(Error::InvalidStructure(format!(
            "expected \"vcard\", got \"{tag}\""
        )));
    }

    let props_arr = arr[1]
        .as_array()
        .ok_or_else(|| {
            Error::InvalidStructure("second element must be a properties array".into())
        })?;

    let properties = parse_properties(props_arr, warnings);

    if properties
        .first()
        .is_none_or(|p| p.name != "version")
    {
        warnings.push(ParseWarning {
            path: "properties[0]".into(),
            message: "expected 'version' as first property".into(),
            raw_value: None,
        });
    }

    Ok(JCard { properties })
}

fn parse_properties(
    raw_props: &[serde_json::Value],
    warnings: &mut Vec<ParseWarning>,
) -> Vec<Property> {
    raw_props
        .iter()
        .enumerate()
        .filter_map(|(i, v)| parse_single_property(i, v, warnings))
        .collect()
}

fn parse_single_property(
    index: usize,
    value: &serde_json::Value,
    warnings: &mut Vec<ParseWarning>,
) -> Option<Property> {
    let arr = match value.as_array() {
        Some(arr) => arr,
        None => {
            warnings.push(ParseWarning {
                path: format!("properties[{index}]"),
                message: format!("property is not an array (got {})", json_type_name(value)),
                raw_value: Some(value.to_string()),
            });
            return None;
        }
    };

    if arr.len() < 4 {
        warnings.push(ParseWarning {
            path: format!("properties[{index}]"),
            message: format!("tuple has {} elements, need at least 4", arr.len()),
            raw_value: Some(value.to_string()),
        });
        return None;
    }

    let name = match arr[0].as_str() {
        Some(s) => s.to_string(),
        None => {
            warnings.push(ParseWarning {
                path: format!("properties[{index}]"),
                message: "name is not a string".into(),
                raw_value: Some(arr[0].to_string()),
            });
            return None;
        }
    };

    let path = format!("properties[{index}] '{name}'");

    let value_type = match arr[2].as_str() {
        Some(s) => s.to_string(),
        None => {
            warnings.push(ParseWarning {
                path: path.clone(),
                message: format!(
                    "type identifier is not a string (got {})",
                    json_type_name(&arr[2])
                ),
                raw_value: Some(arr[2].to_string()),
            });
            "unknown".to_string()
        }
    };

    let parameters = if let Some(map) = arr[1].as_object() {
        parse_parameters(map, &path, warnings)
    } else {
        warnings.push(ParseWarning {
            path: path.clone(),
            message: format!(
                "parameters is not an object (got {})",
                json_type_name(&arr[1])
            ),
            raw_value: Some(arr[1].to_string()),
        });
        BTreeMap::new()
    };

    let values: Vec<PropertyValue> = arr[3..]
        .iter()
        .enumerate()
        .map(|(vi, v)| match PropertyValue::from_json(&value_type, v) {
            Some(pv) => pv,
            None => {
                warnings.push(ParseWarning {
                    path: format!("{path}.values[{vi}]"),
                    message: format!("expected {value_type} value, got {}", json_type_name(v)),
                    raw_value: Some(v.to_string()),
                });
                fallback_text(v)
            }
        })
        .collect();

    if values.is_empty() {
        warnings.push(ParseWarning {
            path,
            message: "no values in property tuple".into(),
            raw_value: None,
        });
        return None;
    }

    Some(Property::from_raw(name, parameters, value_type, values))
}

fn parse_parameters(
    map: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    warnings: &mut Vec<ParseWarning>,
) -> BTreeMap<String, ParamValue> {
    map.iter()
        .filter_map(|(k, v)| {
            let pv = match v {
                serde_json::Value::String(s) => ParamValue::Single(s.clone()),
                serde_json::Value::Array(arr) => {
                    let strings: Vec<String> = arr
                        .iter()
                        .filter_map(|v| {
                            v.as_str()
                                .map(String::from)
                        })
                        .collect();
                    ParamValue::Multiple(strings)
                }
                _ => {
                    warnings.push(ParseWarning {
                        path: format!("{path}.parameters.{k}"),
                        message: format!(
                            "parameter value is not a string or array (got {})",
                            json_type_name(v)
                        ),
                        raw_value: Some(v.to_string()),
                    });
                    return None;
                }
            };
            Some((k.clone(), pv))
        })
        .collect()
}

fn fallback_text(value: &serde_json::Value) -> PropertyValue {
    match value {
        serde_json::Value::String(s) => PropertyValue::Text(s.clone()),
        other => PropertyValue::Text(other.to_string()),
    }
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

impl<'de> Deserialize<'de> for JCard {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let mut warnings = Vec::new();
        match parse_jcard_value(&value, &mut warnings) {
            Ok(jcard) => Ok(jcard),
            Err(_) => Ok(JCard::default()),
        }
    }
}
