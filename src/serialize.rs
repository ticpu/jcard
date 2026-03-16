use serde::ser::{Serialize, SerializeSeq, Serializer};

use crate::property::{ParamValue, Property};
use crate::JCard;

impl Serialize for JCard {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element("vcard")?;
        seq.serialize_element(&PropertiesArray(&self.properties))?;
        seq.end()
    }
}

struct PropertiesArray<'a>(&'a [Property]);

impl Serialize for PropertiesArray<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(
            self.0
                .len(),
        ))?;
        for prop in self.0 {
            seq.serialize_element(&PropertyTuple(prop))?;
        }
        seq.end()
    }
}

struct PropertyTuple<'a>(&'a Property);

impl Serialize for PropertyTuple<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let prop = self.0;
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&prop.name)?;
        seq.serialize_element(&ParametersMap(&prop.parameters))?;
        seq.serialize_element(&prop.value_type)?;
        seq.serialize_element(
            &prop
                .value
                .to_json(),
        )?;
        seq.end()
    }
}

struct ParametersMap<'a>(&'a std::collections::BTreeMap<String, ParamValue>);

impl Serialize for ParametersMap<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(
            self.0
                .len(),
        ))?;
        for (key, val) in self.0 {
            match val {
                ParamValue::Single(s) => map.serialize_entry(key, s)?,
                ParamValue::Multiple(v) => map.serialize_entry(key, v)?,
            }
        }
        map.end()
    }
}
