//! Wire-level helpers shared by create and parse: namespace constants,
//! element-form primitives and the message root attributes.
//!
//! The XMeld wire has three element-form zones, and getting them wrong is
//! the single most likely interoperability bug (the fixtures and the XSD
//! gate both caught it during development):
//!
//! 1. xmeld structural elements are **qualified** — the schemas declare
//!    `elementFormDefault="qualified"`, and instances carry the release
//!    namespace as the default (`http://www.osci.de/xmeld2611` for XMeld
//!    26.11).
//! 2. Leaf data elements defined by the XInneres modules (AllgemeinerName,
//!    Meldeanschrift, TeilbekanntesDatum…) and the xoev-code `<code>`
//!    children are **unqualified** — serialized with `xmlns=""`.
//! 3. The Basisnachricht-g2g leaf elements (kanal, kennung) are
//!    unqualified as well; their wrappers in xmeld stay qualified.

use quick_xml::Writer;
use quick_xml::events::{BytesStart, BytesText, Event};

/// XMeld 26.11 target namespace — the release we create in. Changes with
/// every release; the parser tolerates the historically known ones.
pub const NS_XMELD_2611: &str = "http://www.osci.de/xmeld2611";

/// Release namespaces the parser accepts on 0600/0601/0900 roots. The
/// structure of the Auskunft family is stable across these; anything else
/// is rejected as [crate::Error::UnknownMessage] rather than guessed.
pub const KNOWN_NAMESPACES: [&str; 4] = [
    "http://www.osci.de/xmeld34",
    "http://www.osci.de/xmeld2605",
    NS_XMELD_2611,
    "http://www.osci.de/xmeld2705",
];

/// Release identification carried as root attributes. `fassung` is the
/// spec production date (YYYY-MM-DD), `version` the xMeld release; the
/// defaults describe XMeld 26.11 (Fassung 2026-01-31, per the KoSIT
/// version overview).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootAttributes {
    /// `fassung` root attribute — spec production date, YYYY-MM-DD.
    pub fassung: String,
    /// `version` root attribute — the xMeld release („26.11“).
    pub version: String,
    /// `produkt` root attribute — the software that created the message.
    pub produkt: String,
    /// `produkthersteller` root attribute — the responsible organisation.
    pub produkthersteller: String,
    /// Optional `produktversion`.
    pub produktversion: Option<String>,
    /// Optional `test` marker (set only for test-lab messages).
    pub test: Option<String>,
}

impl Default for RootAttributes {
    fn default() -> Self {
        RootAttributes {
            fassung: "2026-01-31".to_string(),
            version: "26.11".to_string(),
            produkt: "rmeld".to_string(),
            produkthersteller: "rmeld".to_string(),
            produktversion: None,
            test: None,
        }
    }
}

impl RootAttributes {
    /// Push the required-and-optional root attributes onto `start`.
    pub fn push_to(&self, start: &mut BytesStart<'_>) {
        start.push_attribute(("fassung", self.fassung.as_str()));
        start.push_attribute(("version", self.version.as_str()));
        start.push_attribute(("produkt", self.produkt.as_str()));
        start.push_attribute(("produkthersteller", self.produkthersteller.as_str()));
        if let Some(v) = &self.produktversion {
            start.push_attribute(("produktversion", v.as_str()));
        }
        if let Some(t) = &self.test {
            start.push_attribute(("test", t.as_str()));
        }
    }
}

/// A qualified xmeld child element (zone 1 — inherits the default
/// namespace).
pub fn q(name: &'static str) -> BytesStart<'static> {
    BytesStart::new(name)
}

/// An unqualified leaf element (zones 2/3 — `xmlns=""` on the instance).
pub fn u(name: &'static str) -> BytesStart<'static> {
    let mut start = BytesStart::new(name);
    start.push_attribute(("xmlns", ""));
    start
}

/// Write `<{start}>{text}</{name}>` in one go.
pub fn write_text_element(
    writer: &mut Writer<&mut Vec<u8>>,
    start: BytesStart<'_>,
    text: &str,
) -> Result<(), crate::Error> {
    use quick_xml::events::BytesEnd;
    writer.write_event(Event::Start(start.clone()))?;
    writer.write_event(Event::Text(BytesText::new(text)))?;
    let end_name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    writer.write_event(Event::End(BytesEnd::new(end_name)))?;
    Ok(())
}

/// Write an empty element `<{start}/>`.
pub fn write_empty_element(
    writer: &mut Writer<&mut Vec<u8>>,
    start: BytesStart<'_>,
) -> Result<(), crate::Error> {
    writer.write_event(Event::Empty(start))?;
    Ok(())
}

/// Write an xoev code element: a wrapper element (antwortstatus,
/// nachrichtennummer, ergebnisstatus, …) with the unqualified `code`
/// child carrying the value. The caller builds the wrapper — qualified
/// ([`q`] for xmeld code types) or unqualified ([`u`] for XInneres/g2g
/// codes like `gemeindeschluessel`/`kanal`) — and pushes `listVersionID`
/// where the type requires it (XInneres AGS code: required; xmeld code
/// types: optional).
pub fn write_code_element(
    writer: &mut Writer<&mut Vec<u8>>,
    wrapper: BytesStart<'_>,
    code: &str,
) -> Result<(), crate::Error> {
    writer.write_event(Event::Start(wrapper.clone()))?;
    write_text_element(writer, u("code"), code)?;
    let end_name = String::from_utf8_lossy(wrapper.name().as_ref()).into_owned();
    writer.write_event(Event::End(quick_xml::events::BytesEnd::new(end_name)))?;
    Ok(())
}
