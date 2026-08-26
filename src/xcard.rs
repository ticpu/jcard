//! RFC 6351 xCard — the XML representation of vCard, read into [`JCard`].
//!
//! ```
//! # use jcard::xcard;
//! let parsed = xcard::parse(
//!     r#"<vcards xmlns="urn:ietf:params:xml:ns:vcard-4.0">
//!          <vcard><fn><text>Jane Doe</text></fn></vcard>
//!        </vcards>"#,
//! )
//! .unwrap();
//! assert_eq!(parsed.value.len(), 1);
//! ```

use std::collections::BTreeMap;

use quick_xml::escape::resolve_xml_entity;
use quick_xml::events::{BytesEnd, BytesRef, BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;

use crate::error::Error;
use crate::property::{ParamValue, Property, PropertyValue, StructuredComponent};
use crate::{JCard, ParseWarning, Parsed};

/// Properties whose children are named components of one structured value,
/// in jCard array order (RFC 6351 Appendix A).
const COMPONENT_PROPERTIES: &[(&str, &[&str])] = &[
    (
        "adr",
        &[
            "pobox", "ext", "street", "locality", "region", "code", "country",
        ],
    ),
    ("clientpidmap", &["sourceid", "uri"]),
    ("gender", &["sex", "identity"]),
    ("n", &["surname", "given", "additional", "prefix", "suffix"]),
];

/// Properties whose repeated value elements are components of one structured
/// value.
const LIST_PROPERTIES: &[&str] = &["org"];

/// Properties whose repeated value elements are separate values of the
/// property (RFC 7095 §3.3).
const MULTI_PROPERTIES: &[&str] = &["categories", "nickname"];

/// Reads an xCard document into one [`JCard`] per `<vcard>` element.
///
/// `Err` is reserved for input that yields no card at all — ill-formed XML, or
/// markup containing no `<vcard>`. Everything else is best-effort: malformed
/// properties, values that do not match their declared type, and a missing
/// `<vcards>` wrapper all come back as [`ParseWarning`] entries alongside the
/// cards.
///
/// The input may be a fragment lifted out of a larger document: elements are
/// matched on their local name, so namespace prefixes bound on an ancestor
/// that is not present do not prevent parsing.
pub fn parse(xml: &str) -> Result<Parsed<Vec<JCard>>, Error> {
    let mut ctx = Ctx::new(xml);
    let mut cards = Vec::new();

    loop {
        match ctx.read_event()? {
            Event::Start(e) => match e
                .local_name()
                .as_ref()
            {
                "vcards" => ctx.read_vcards(&mut cards)?,
                "vcard" => {
                    let path = format!("vcard[{}]", cards.len());
                    ctx.recovered(&path, "no <vcards> wrapper element", None);
                    let card = ctx.read_vcard(cards.len())?;
                    cards.push(card);
                }
                _ => ctx.skip_element()?,
            },
            Event::Eof => break,
            _ => {}
        }
    }

    if cards.is_empty() {
        return Err(Error::InvalidStructure("no <vcard> element".into()));
    }
    Ok(Parsed {
        value: cards,
        warnings: ctx.warnings,
    })
}

struct Ctx<'a> {
    reader: Reader<&'a [u8]>,
    warnings: Vec<ParseWarning>,
    /// End tag synthesized for an empty-element tag, returned by the next
    /// [`Self::read_event`].
    pending_end: Option<BytesEnd<'static>>,
}

impl<'a> Ctx<'a> {
    fn new(xml: &'a str) -> Self {
        let mut reader = Reader::from_str(xml);
        // A lone `&` is ill-formed, but refusing the card over one character
        // loses every property in it; `char_data` reports it instead.
        reader
            .config_mut()
            .allow_dangling_amp = true;
        Self {
            reader,
            warnings: Vec::new(),
            pending_end: None,
        }
    }

    /// Reads the next event, expanding `<x/>` into the `Start`/`End` pair so
    /// callers need only one shape.
    fn read_event(&mut self) -> Result<Event<'a>, Error> {
        if let Some(end) = self
            .pending_end
            .take()
        {
            return Ok(Event::End(end));
        }
        match self
            .reader
            .read_event()
            .map_err(xml_err)?
        {
            Event::Empty(e) => {
                self.pending_end = Some(
                    e.to_end()
                        .into_owned(),
                );
                Ok(Event::Start(e))
            }
            other => Ok(other),
        }
    }

    /// Reports a value that did not survive the problem.
    fn lost(&mut self, path: &str, message: &str, raw_value: Option<String>) {
        self.warnings
            .push(ParseWarning::lost(path, message, raw_value));
    }

    /// Reports non-conformant input the value came through intact.
    fn recovered(&mut self, path: &str, message: &str, raw_value: Option<String>) {
        self.warnings
            .push(ParseWarning::recovered(path, message, raw_value));
    }

    /// Every loop below fully consumes each child it opens, so the first `End`
    /// it sees closes the element it was entered for.
    fn read_vcards(&mut self, cards: &mut Vec<JCard>) -> Result<(), Error> {
        loop {
            match self.read_event()? {
                Event::Start(e) => {
                    if e.local_name()
                        .as_ref()
                        == "vcard"
                    {
                        let card = self.read_vcard(cards.len())?;
                        cards.push(card);
                    } else {
                        self.skip_element()?;
                    }
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }
        Ok(())
    }

    fn read_vcard(&mut self, index: usize) -> Result<JCard, Error> {
        // xCard has no version property: the namespace carries it.
        let mut properties = vec![Property::new(
            "version",
            PropertyValue::Text("4.0".to_owned()),
        )];

        loop {
            match self.read_event()? {
                Event::Start(e) => {
                    let name = element_name(&e);
                    if name == "group" {
                        let path = format!("vcard[{index}] 'group'");
                        let group = self.attr_text(&e, "name", &path);
                        if group.is_none() {
                            self.lost(&path, "group element has no name attribute", None);
                        }
                        self.read_group(index, group.as_deref(), &mut properties)?;
                    } else {
                        let property = self.read_property(index, &name, None)?;
                        properties.push(property);
                    }
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }
        Ok(JCard { properties })
    }

    fn read_group(
        &mut self,
        index: usize,
        group: Option<&str>,
        out: &mut Vec<Property>,
    ) -> Result<(), Error> {
        loop {
            match self.read_event()? {
                Event::Start(e) => {
                    let name = element_name(&e);
                    if name == "group" {
                        self.lost(
                            &format!("vcard[{index}] 'group'"),
                            "nested group element discarded",
                            None,
                        );
                        self.skip_element()?;
                    } else {
                        let property = self.read_property(index, &name, group)?;
                        out.push(property);
                    }
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }
        Ok(())
    }

    fn read_property(
        &mut self,
        index: usize,
        name: &str,
        group: Option<&str>,
    ) -> Result<Property, Error> {
        let path = format!("vcard[{index}] '{name}'");
        let mut parameters = BTreeMap::new();
        if let Some(group) = group {
            parameters.insert(
                "group".to_owned(),
                ParamValue::Single(group.to_ascii_lowercase()),
            );
        }

        let components = COMPONENT_PROPERTIES
            .iter()
            .find(|(property, _)| *property == name)
            .map(|(_, components)| *components);
        let mut slots: Vec<Vec<String>> = components
            .map(|c| vec![Vec::new(); c.len()])
            .unwrap_or_default();
        let mut values: Vec<(String, PropertyValue)> = Vec::new();
        let mut loose_text = String::new();

        loop {
            let event = self.read_event()?;
            if let Some(text) = self.char_data(&path, &event) {
                loose_text.push_str(&text);
                continue;
            }
            match event {
                Event::Start(e) => {
                    let child = element_name(&e);
                    if child == "parameters" {
                        self.read_parameters(&path, &mut parameters)?;
                        continue;
                    }
                    let position = components.and_then(|c| {
                        c.iter()
                            .position(|n| *n == child)
                    });
                    let text = self.read_text(&path)?;
                    match position {
                        Some(position) => slots[position].push(text),
                        None => match PropertyValue::from_typed_text(&child, &text) {
                            Some(value) => values.push((child, value)),
                            None => {
                                // The text is kept verbatim, and `value_type`
                                // still reports what the sender declared.
                                self.recovered(
                                    &path,
                                    &format!("value does not parse as declared type '{child}'"),
                                    Some(text.clone()),
                                );
                                values.push((child, PropertyValue::Unknown(text)));
                            }
                        },
                    }
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }

        if components.is_some() {
            if slots
                .iter()
                .all(Vec::is_empty)
            {
                self.lost(&path, "structured property has no component element", None);
            }
            let value = PropertyValue::Structured(
                slots
                    .into_iter()
                    .map(component)
                    .collect(),
            );
            return Ok(Property::from_raw(
                name.to_owned(),
                parameters,
                "text".to_owned(),
                vec![value],
            ));
        }

        if values.is_empty() {
            let text = loose_text
                .trim()
                .to_owned();
            if text.is_empty() {
                self.lost(&path, "property has no value element", None);
            } else {
                self.recovered(
                    &path,
                    "value text is not wrapped in a value element",
                    Some(text.clone()),
                );
            }
            return Ok(Property::from_raw(
                name.to_owned(),
                parameters,
                "unknown".to_owned(),
                vec![PropertyValue::Unknown(text)],
            ));
        }

        if LIST_PROPERTIES.contains(&name) && values.len() > 1 {
            let value = PropertyValue::Structured(
                values
                    .iter()
                    .map(|(_, v)| StructuredComponent::Text(v.to_string()))
                    .collect(),
            );
            return Ok(Property::from_raw(
                name.to_owned(),
                parameters,
                "text".to_owned(),
                vec![value],
            ));
        }

        if values.len() > 1 && !MULTI_PROPERTIES.contains(&name) {
            // Every value is kept; only the shape is non-conformant.
            self.recovered(&path, "property has more than one value element", None);
        }

        let value_type = values[0]
            .0
            .clone();
        let values = values
            .into_iter()
            .map(|(_, value)| value)
            .collect();
        Ok(Property::from_raw(
            name.to_owned(),
            parameters,
            value_type,
            values,
        ))
    }

    fn read_parameters(
        &mut self,
        path: &str,
        out: &mut BTreeMap<String, ParamValue>,
    ) -> Result<(), Error> {
        loop {
            match self.read_event()? {
                Event::Start(e) => {
                    let key = element_name(&e);
                    let values = self.read_param_values(path, &key)?;
                    let value = ParamValue::try_from(values).unwrap_or_else(|_| {
                        self.lost(path, &format!("parameter '{key}' has no value"), None);
                        ParamValue::Single(String::new())
                    });
                    if out
                        .insert(key.clone(), value)
                        .is_some()
                    {
                        self.lost(
                            path,
                            &format!("parameter '{key}' given more than once; last one kept"),
                            None,
                        );
                    }
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }
        Ok(())
    }

    fn read_param_values(&mut self, path: &str, key: &str) -> Result<Vec<String>, Error> {
        let mut values = Vec::new();
        let mut loose_text = String::new();

        loop {
            let event = self.read_event()?;
            if let Some(text) = self.char_data(path, &event) {
                loose_text.push_str(&text);
                continue;
            }
            match event {
                Event::Start(_) => {
                    let text = self.read_text(path)?;
                    values.push(text);
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }

        let loose_text = loose_text.trim();
        if values.is_empty() && !loose_text.is_empty() {
            self.recovered(
                path,
                &format!("parameter '{key}' value is not wrapped in a value element"),
                Some(loose_text.to_owned()),
            );
            values.push(loose_text.to_owned());
        }
        Ok(values)
    }

    /// Accumulates the character data of the element that is currently open.
    fn read_text(&mut self, path: &str) -> Result<String, Error> {
        let mut text = String::new();
        loop {
            let event = self.read_event()?;
            if let Some(chunk) = self.char_data(path, &event) {
                text.push_str(&chunk);
                continue;
            }
            match event {
                Event::Start(e) => {
                    let child = element_name(&e);
                    self.lost(
                        path,
                        &format!("child element <{child}> discarded from a value element"),
                        None,
                    );
                    self.skip_element()?;
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
        }
        Ok(text)
    }

    /// Character content carried by an event, `None` for events that carry
    /// none.
    fn char_data(&mut self, path: &str, event: &Event<'_>) -> Option<String> {
        // No declared version is tracked and xCard is XML 1.0, so end-of-line
        // normalization follows XML 1.0 rules.
        match event {
            Event::Text(e) => Some(
                e.xml_content(XmlVersion::Implicit1_0)
                    .into_owned(),
            ),
            Event::CData(e) => Some(
                e.xml_content(XmlVersion::Implicit1_0)
                    .into_owned(),
            ),
            Event::GeneralRef(e) => {
                let body = e.as_ref();
                Some(match resolve_reference(body) {
                    Some(resolved) => resolved,
                    None => {
                        let literal = format!("&{body};");
                        self.lost(path, "unresolvable entity reference", Some(literal.clone()));
                        literal
                    }
                })
            }
            _ => None,
        }
    }

    fn attr_text(&mut self, e: &BytesStart<'_>, key: &str, path: &str) -> Option<String> {
        for attr in e
            .attributes()
            .flatten()
        {
            if attr
                .key
                .local_name()
                .as_ref()
                != key
            {
                continue;
            }
            return match attr.normalized_value(XmlVersion::Implicit1_0) {
                Ok(value) => Some(value.into_owned()),
                Err(_) => {
                    let raw = attr
                        .value
                        .into_owned();
                    self.lost(
                        path,
                        "attribute holds an unresolvable reference",
                        Some(raw.clone()),
                    );
                    Some(raw)
                }
            };
        }
        None
    }

    fn skip_element(&mut self) -> Result<(), Error> {
        let mut depth = 1u32;
        loop {
            match self.read_event()? {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(())
    }
}

fn component(texts: Vec<String>) -> StructuredComponent {
    match texts.len() {
        1 => StructuredComponent::Text(
            texts
                .into_iter()
                .next()
                .unwrap_or_default(),
        ),
        0 => StructuredComponent::Text(String::new()),
        _ => StructuredComponent::Multi(texts),
    }
}

fn xml_err(e: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::InvalidXml(Box::new(e))
}

/// Local element name, lowercased per RFC 6351 §5.1.
fn element_name(e: &BytesStart<'_>) -> String {
    e.local_name()
        .as_ref()
        .to_ascii_lowercase()
}

/// Resolves a reference body — the text between `&` and `;` — to its content.
///
/// `None` for a general entity, whose definition lives in a DTD this crate
/// does not process, and for a malformed character reference.
///
/// `resolve_xml_entity` rather than `resolve_predefined_entity`: the latter
/// widens to the HTML5 set if anything in the dependency graph turns on
/// quick-xml's `escape-html`, which would make resolution build-dependent.
fn resolve_reference(body: &str) -> Option<String> {
    match BytesRef::new(body).resolve_char_ref() {
        Ok(Some(ch)) => Some(ch.to_string()),
        Ok(None) => resolve_xml_entity(body).map(str::to_owned),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6351 §4, with the author's contact details replaced.
    const RFC6351_CARD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<vcards xmlns="urn:ietf:params:xml:ns:vcard-4.0">
  <vcard>
    <fn><text>Jane Doe</text></fn>
    <n>
      <surname>Doe</surname>
      <given>Jane</given>
      <additional/>
      <prefix/>
      <suffix>ing. jr</suffix>
      <suffix>M.Sc.</suffix>
    </n>
    <bday><date>--0203</date></bday>
    <anniversary>
      <date-time>20090808T1430-0500</date-time>
    </anniversary>
    <gender><sex>F</sex></gender>
    <lang>
      <parameters><pref><integer>1</integer></pref></parameters>
      <language-tag>fr</language-tag>
    </lang>
    <lang>
      <parameters><pref><integer>2</integer></pref></parameters>
      <language-tag>en</language-tag>
    </lang>
    <org>
      <parameters><type><text>work</text></type></parameters>
      <text>EXAMPLE CO</text>
    </org>
    <adr>
      <parameters>
        <type><text>work</text></type>
      </parameters>
      <pobox/>
      <ext/>
      <street>123 Main Street</street>
      <locality>Any Town</locality>
      <region>QC</region>
      <code>G1V 2M2</code>
      <country>Canada</country>
    </adr>
    <tel>
      <parameters>
        <type>
          <text>work</text>
          <text>voice</text>
        </type>
      </parameters>
      <uri>tel:+15550101</uri>
    </tel>
    <email>
      <parameters><type><text>work</text></type></parameters>
      <text>jane.doe@example.com</text>
    </email>
    <categories><text>computers</text><text>cameras</text></categories>
    <tz><text>America/Montreal</text></tz>
  </vcard>
</vcards>"#;

    fn only_card(xml: &str) -> Parsed<JCard> {
        let parsed = parse(xml).expect("parses");
        assert_eq!(
            parsed
                .value
                .len(),
            1,
            "expected exactly one card"
        );
        Parsed {
            value: parsed
                .value
                .into_iter()
                .next()
                .expect("one card"),
            warnings: parsed.warnings,
        }
    }

    #[test]
    fn rfc6351_example_card() {
        let parsed = only_card(RFC6351_CARD);
        assert!(
            parsed
                .warnings
                .is_empty(),
            "{:?}",
            parsed.warnings
        );
        let card = parsed.value;

        // xCard has no version property; jCard requires one first.
        assert_eq!(card.properties()[0].name, "version");
        assert_eq!(
            *card.properties()[0].value(),
            PropertyValue::Text("4.0".into())
        );

        assert_eq!(
            *card
                .get("fn")
                .unwrap()
                .value(),
            PropertyValue::Text("Jane Doe".into())
        );

        // Named components are positional, and a repeated one is a nested list.
        assert_eq!(
            *card
                .get("n")
                .unwrap()
                .value(),
            PropertyValue::Structured(vec![
                StructuredComponent::Text("Doe".into()),
                StructuredComponent::Text("Jane".into()),
                StructuredComponent::Text(String::new()),
                StructuredComponent::Text(String::new()),
                StructuredComponent::Multi(vec!["ing. jr".into(), "M.Sc.".into()]),
            ])
        );

        assert_eq!(
            *card
                .get("adr")
                .unwrap()
                .value(),
            PropertyValue::Structured(vec![
                StructuredComponent::Text(String::new()),
                StructuredComponent::Text(String::new()),
                StructuredComponent::Text("123 Main Street".into()),
                StructuredComponent::Text("Any Town".into()),
                StructuredComponent::Text("QC".into()),
                StructuredComponent::Text("G1V 2M2".into()),
                StructuredComponent::Text("Canada".into()),
            ])
        );

        let langs = card.get_all("lang");
        assert_eq!(langs.len(), 2);
        assert_eq!(*langs[0].value(), PropertyValue::LanguageTag("fr".into()));
        assert_eq!(
            langs[0]
                .parameters
                .get("pref"),
            Some(&ParamValue::Single("1".into()))
        );
        assert_eq!(*langs[1].value(), PropertyValue::LanguageTag("en".into()));

        let tel = card
            .get("tel")
            .unwrap();
        assert_eq!(*tel.value(), PropertyValue::Uri("tel:+15550101".into()));
        assert_eq!(
            tel.parameters
                .get("type"),
            Some(&ParamValue::Multiple(vec!["work".into(), "voice".into()]))
        );

        // categories is multi-valued, org is a single structured value.
        assert_eq!(
            card.get("categories")
                .unwrap()
                .values(),
            &[
                PropertyValue::Text("computers".into()),
                PropertyValue::Text("cameras".into()),
            ]
        );
        assert_eq!(
            *card
                .get("org")
                .unwrap()
                .value(),
            PropertyValue::Text("EXAMPLE CO".into())
        );

        assert_eq!(
            *card
                .get("bday")
                .unwrap()
                .value(),
            PropertyValue::Date("--0203".into())
        );
        assert_eq!(
            *card
                .get("anniversary")
                .unwrap()
                .value(),
            PropertyValue::DateTime("20090808T1430-0500".into())
        );
        assert_eq!(
            *card
                .get("gender")
                .unwrap()
                .value(),
            PropertyValue::Structured(vec![
                StructuredComponent::Text("F".into()),
                StructuredComponent::Text(String::new()),
            ])
        );
    }

    #[test]
    fn multi_component_org_is_one_structured_value() {
        let parsed = only_card(
            r#"<vcards><vcard><org><text>EXAMPLE CO</text><text>Field Ops</text></org></vcard></vcards>"#,
        );
        assert!(
            parsed
                .warnings
                .is_empty(),
            "{:?}",
            parsed.warnings
        );
        assert_eq!(
            *parsed
                .value
                .get("org")
                .unwrap()
                .value(),
            PropertyValue::Structured(vec![
                StructuredComponent::Text("EXAMPLE CO".into()),
                StructuredComponent::Text("Field Ops".into()),
            ])
        );
    }

    #[test]
    fn fragment_without_wrapper_or_namespace_binding() {
        let parsed = parse(
            r#"<vcard:vcard>
                 <vcard:fn><vcard:text>Jane Doe</vcard:text></vcard:fn>
                 <vcard:lang><vcard:language-tag>FR</vcard:language-tag></vcard:lang>
               </vcard:vcard>"#,
        )
        .expect("parses");

        // The card came through whole, so the omission must not read as data
        // the reader lost.
        assert_eq!(
            parsed
                .warnings
                .iter()
                .map(|w| (
                    w.kind,
                    w.message
                        .as_str()
                ))
                .collect::<Vec<_>>(),
            [(crate::WarningKind::Recovered, "no <vcards> wrapper element")]
        );
        let card = &parsed.value[0];
        assert_eq!(
            *card
                .get("fn")
                .unwrap()
                .value(),
            PropertyValue::Text("Jane Doe".into())
        );
        // The tag's case is what the sender wrote; BCP 47 comparison is the
        // consumer's job.
        assert_eq!(
            *card
                .get("lang")
                .unwrap()
                .value(),
            PropertyValue::LanguageTag("FR".into())
        );
    }

    #[test]
    fn value_that_contradicts_its_declared_type_survives() {
        let parsed = only_card(
            r#"<vcards><vcard><x-count><integer>lots</integer></x-count></vcard></vcards>"#,
        );
        let property = parsed
            .value
            .get("x-count")
            .unwrap();
        assert_eq!(property.value_type, "integer");
        assert_eq!(*property.value(), PropertyValue::Unknown("lots".into()));
        assert_eq!(parsed.warnings[0].raw_value, Some("lots".into()));
    }

    #[test]
    fn group_becomes_a_parameter() {
        let parsed = only_card(
            r#"<vcards><vcard>
                 <group name="Contact"><fn><text>Jane Doe</text></fn></group>
                 <categories><text>staff</text></categories>
               </vcard></vcards>"#,
        );
        assert!(
            parsed
                .warnings
                .is_empty(),
            "{:?}",
            parsed.warnings
        );
        assert_eq!(
            parsed
                .value
                .get("fn")
                .unwrap()
                .parameters
                .get("group"),
            Some(&ParamValue::Single("contact".into()))
        );
        assert!(parsed
            .value
            .get("categories")
            .unwrap()
            .parameters
            .is_empty());
    }

    #[test]
    fn value_text_outside_a_value_element_is_recovered_and_reported() {
        let parsed = only_card(r#"<vcards><vcard><fn>Jane Doe</fn></vcard></vcards>"#);
        let property = parsed
            .value
            .get("fn")
            .unwrap();
        assert_eq!(*property.value(), PropertyValue::Unknown("Jane Doe".into()));
        assert_eq!(
            parsed.warnings[0].message,
            "value text is not wrapped in a value element"
        );
    }

    #[test]
    fn markup_without_a_card_is_an_error() {
        assert!(parse("<SubscriberData>Jane Doe</SubscriberData>").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn several_cards_in_one_document() {
        let parsed = parse(
            r#"<vcards>
                 <vcard><fn><text>Jane Doe</text></fn></vcard>
                 <vcard><fn><text>John Doe</text></fn></vcard>
               </vcards>"#,
        )
        .expect("parses");
        assert_eq!(
            parsed
                .value
                .len(),
            2
        );
        assert!(
            parsed
                .warnings
                .is_empty(),
            "{:?}",
            parsed.warnings
        );
    }

    #[test]
    fn entity_references_resolve_in_element_text() {
        let parsed = only_card(
            r#"<vcards><vcard><note><text>Doe &amp; Sons &#x2014; call &lt;first&gt;</text></note></vcard></vcards>"#,
        );
        assert_eq!(
            *parsed
                .value
                .get("note")
                .unwrap()
                .value(),
            PropertyValue::Text("Doe & Sons — call <first>".into())
        );
    }
}
