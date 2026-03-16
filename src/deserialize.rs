use serde::de::{self, Deserialize, Deserializer, SeqAccess, Visitor};

use crate::property::{ParamValue, Property, PropertyValue};
use crate::JCard;

impl<'de> Deserialize<'de> for JCard {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(JCardVisitor)
    }
}

struct JCardVisitor;

impl<'de> Visitor<'de> for JCardVisitor {
    type Value = JCard;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(r#"a jCard array: ["vcard", [...properties]]"#)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let tag: String = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        if tag != "vcard" {
            return Err(de::Error::invalid_value(
                de::Unexpected::Str(&tag),
                &"\"vcard\"",
            ));
        }

        let props: Vec<RawProperty> = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;

        let properties = props
            .into_iter()
            .filter_map(|raw| raw.into_property())
            .collect();

        Ok(JCard { properties })
    }
}

struct RawProperty(Vec<serde_json::Value>);

impl<'de> Deserialize<'de> for RawProperty {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let arr = Vec::<serde_json::Value>::deserialize(deserializer)?;
        Ok(RawProperty(arr))
    }
}

impl RawProperty {
    fn into_property(self) -> Option<Property> {
        let arr = self.0;
        if arr.len() < 4 {
            return None;
        }

        let name = arr[0]
            .as_str()?
            .to_string();
        let params_value = &arr[1];
        let value_type = arr[2]
            .as_str()?
            .to_string();

        let parameters = match params_value.as_object() {
            Some(map) => map
                .iter()
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
                        _ => return None,
                    };
                    Some((k.clone(), pv))
                })
                .collect(),
            None => std::collections::BTreeMap::new(),
        };

        let values: Vec<PropertyValue> = arr[3..]
            .iter()
            .filter_map(|v| PropertyValue::from_json(&value_type, v))
            .collect();
        if values.is_empty() {
            return None;
        }

        Some(Property::from_raw(name, parameters, value_type, values))
    }
}
