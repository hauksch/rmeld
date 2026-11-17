//! Create Nachricht 0600 („Anfrage zur einfachen Melderegisterauskunft“)
//! from structured input.
//!
//! The JSON contract ([`AnfrageInput`]) is what the Fachanwendung hands
//! over; the XSD gate (crate::validate) proves the emitted XML against the
//! official XMeld 26.11 schemas. Deliberately narrow: phase 1 supports the
//! name + Anschrift + Geburtsdatum search triple and the mandatory
//! Werbung/Adresshandel declaration. The schema's rarer Suchprofil
//! criteria (Ordensname, Partnername, Sterbetag, …) are phase-2 material —
//! absent support is a documented limit, not silent dropping.

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, Event};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::wire::{NS_XMELD_2611, RootAttributes, q, u, write_code_element, write_text_element};

/// Structured input for one or more Einzelanfragen, serialized to a 0600
/// Sammelnachricht. The three known-release fields (`fassung`, `version`,
/// `ereigniszeitpunkt`) default sensibly; everything else is required and
/// fails fast.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnfrageInput {
    /// Correlation ID for the whole message (zeichennachricht, echoed in
    /// the Antwort). Aktenzeichen, Surrogat, Geschäftskennzeichen.
    pub zeichennachricht: String,
    /// Empfangende Meldebehörde, identified by its Behördenkennung (AGS
    /// ''schlüsselnorm: 13 digits in the fixtures; DVDV supplies the real
    /// value in production).
    pub empfaenger_behoerdenkennung: String,
    /// Name of the anfragende Stelle (juristische Person), 1..n
    /// Einzelanfragen.
    pub absender_bezeichnung: String,
    /// Einzelanfragen; at least one.
    pub anfragen: Vec<EinzelanfrageInput>,
    /// Zeitpunkt of the Ereignis per Einzelanfrage (technische
    /// einzelidentifikation/ereigniszeitpunkt). Defaults to now.
    #[serde(default)]
    pub ereigniszeitpunkt: Option<String>,
    /// Root attribute overrides; defaults describe XMeld 26.11.
    #[serde(default)]
    pub attribute: RootAttributesInput,
}

/// Root attribute overrides ([crate::wire::RootAttributes], JSON view).
/// Every field defaults to the XMeld 26.11 values when absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RootAttributesInput {
    /// Override `fassung` (spec production date, YYYY-MM-DD).
    #[serde(default)]
    pub fassung: Option<String>,
    /// Override `version` (the xMeld release).
    #[serde(default)]
    pub version: Option<String>,
    /// Override `produkt`.
    #[serde(default)]
    pub produkt: Option<String>,
    /// Override `produkthersteller`.
    #[serde(default)]
    pub produkthersteller: Option<String>,
    /// Set optional `produktversion`.
    #[serde(default)]
    pub produktversion: Option<String>,
    /// Set optional `test` marker.
    #[serde(default)]
    pub test: Option<String>,
}

/// One Einzelanfrage (one Suchprofil) inside the 0600 Sammelnachricht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EinzelanfrageInput {
    /// Correlation ID for this Einzelanfrage (zeicheneinzelfall), echoed
    /// per Auskunft position. Optional in the JSON — but **mandatory on
    /// the wire** (technische.einzelidentifikation), so when absent rmeld
    /// emits `{zeichennachricht}-{1-based position}` deterministically.
    #[serde(default)]
    pub zeicheneinzelfall: Option<String>,
    /// Familienname (Suchprofil.name/nachname). Required.
    pub nachname: String,
    /// Vornamen (Suchprofil.name/vornamen). Required.
    pub vornamen: String,
    /// Geburtsdatum, fully or partially known: `YYYY-MM-DD`, `YYYY-MM`,
    /// `YYYY`, or `"unbekannt"`. Optional — but the fachlich Suchregel
    /// („mindestens eine Anschrift, oder zwei der weiteren zulässigen
    /// Daten“) keeps the Anschrift mandatory in phase 1, so the effective
    /// Suchprofil is name + Anschrift (+ Geburtsdatum).
    #[serde(default)]
    pub geburtsdatum: Option<PartialDatum>,
    /// Anschrift des Suchprofils (Inland). Required in phase 1 — see the
    /// Suchregel note on [EinzelanfrageInput::geburtsdatum].
    pub anschrift: AnschriftInput,
}

/// Partially known date (xida:TeilbekanntesDatumMitUnbekannt wire shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct PartialDatum {
    /// Which partial-date element this serializes to.
    pub granularity: Granularity,
    /// The raw value („1968-03-17“, „1968-03“, „1968“) or „unbekannt“.
    pub value: String,
}

/// Which partial-date element a [PartialDatum] serializes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Granularity {
    /// Full date → `jahrMonatTag` (YYYY-MM-DD).
    Tag,
    /// Year-month → `jahrMonat` (YYYY-MM).
    Monat,
    /// Year only → `jahr` (YYYY).
    Jahr,
}

impl Granularity {
    fn element_name(self) -> &'static str {
        match self {
            Granularity::Tag => "jahrMonatTag",
            Granularity::Monat => "jahrMonat",
            Granularity::Jahr => "jahr",
        }
    }
}

impl TryFrom<String> for PartialDatum {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let part = |x: &str, low: usize, high: usize| {
            (x.len() == low || x.len() == high) && x.bytes().all(|b| b.is_ascii_digit())
        };
        let year_ok = |y: &str| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit());
        let s = s.trim().to_string();
        if s == "unbekannt" {
            // Handled via an explicit marker the serializer maps to the
            // unbekannt element.
            return Ok(PartialDatum {
                granularity: Granularity::Jahr,
                value: "unbekannt".to_string(),
            });
        }
        let parts: Vec<&str> = s.split('-').collect();
        let ok = match parts.as_slice() {
            [y] => year_ok(y),
            [y, m] => year_ok(y) && part(m, 1, 2),
            [y, m, d] => year_ok(y) && part(m, 1, 2) && part(d, 1, 2),
            _ => false,
        };
        if !ok {
            return Err(format!(
                "geburtsdatum must be YYYY-MM-DD, YYYY-MM, YYYY or \"unbekannt\", got {s:?}"
            ));
        }
        let granularity = match parts.len() {
            3 => Granularity::Tag,
            2 => Granularity::Monat,
            _ => Granularity::Jahr,
        };
        Ok(PartialDatum {
            granularity,
            value: s,
        })
    }
}

impl std::str::FromStr for PartialDatum {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PartialDatum::try_from(s.to_string())
    }
}

impl PartialDatum {
    fn is_unbekannt(&self) -> bool {
        self.value == "unbekannt"
    }
}

/// Suchprofil Anschrift (Inland form). Postleitzahl/Strasse/Wohnort are
/// the required minimum on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnschriftInput {
    /// Amtlicher Gemeindeschlüssel, if known (gemeindeschluessel code).
    #[serde(default)]
    pub ags: Option<String>,
    /// hausnummer, if the search knows it.
    #[serde(default)]
    pub hausnummer: Option<String>,
    /// postleitzahl — required on the wire.
    pub postleitzahl: String,
    /// strasse — required on the wire.
    pub strasse: String,
    /// wohnort — required on the wire.
    pub wohnort: String,
}

/// A built 0600 message, ready to serialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anfrage {
    /// Resolved root attributes (defaults applied).
    pub attribute: RootAttributes,
    /// The validated input the message was built from.
    pub input: AnfrageInput,
}

impl Anfrage {
    /// Validate the fachlich invariants the XSD cannot express and build
    /// the message. Fail-fast per house rules: every problem is a hard
    /// error, never a warning.
    pub fn build(input: AnfrageInput) -> Result<Anfrage, Error> {
        if input.anfragen.is_empty() {
            return Err(Error::InvalidRequest(
                "at least one Einzelanfrage is required (auskunft.anforderung maxOccurs=unbounded, minOccurs=1)",
            ));
        }
        for anfrage in &input.anfragen {
            if anfrage.anschrift.postleitzahl.is_empty()
                || anfrage.anschrift.strasse.is_empty()
                || anfrage.anschrift.wohnort.is_empty()
            {
                return Err(Error::InvalidRequest(
                    "Anschrift requires postleitzahl, strasse and wohnort (the phase-1 Suchprofil keeps the Anschrift mandatory: „mindestens eine Anschrift, oder zwei der weiteren zulässigen Daten“)",
                ));
            }
        }
        if input.empfaenger_behoerdenkennung.trim().is_empty() {
            return Err(Error::InvalidRequest(
                "empfaenger_behoerdenkennung must not be empty (nachrichtenkopf/empfaenger/behoerdenkennung is mandatory)",
            ));
        }
        let defaults = RootAttributes::default();
        let attr = &input.attribute;
        let attribute = RootAttributes {
            fassung: attr
                .fassung
                .clone()
                .unwrap_or_else(|| defaults.fassung.clone()),
            version: attr
                .version
                .clone()
                .unwrap_or_else(|| defaults.version.clone()),
            produkt: attr
                .produkt
                .clone()
                .unwrap_or_else(|| defaults.produkt.clone()),
            produkthersteller: attr
                .produkthersteller
                .clone()
                .unwrap_or_else(|| defaults.produkthersteller.clone()),
            produktversion: attr.produktversion.clone(),
            test: attr.test.clone(),
        };
        Ok(Anfrage { attribute, input })
    }

    /// Serialize to xMeld XML (UTF-8, declaration, indented).
    pub fn to_xml(&self) -> Result<String, Error> {
        let mut buf = Vec::with_capacity(2048);
        let mut writer = Writer::new_with_indent(&mut buf, b' ', 2);
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            None,
        )))?;

        let mut root = q("melderegisterauskunfteinfach.anforderung.0600");
        root.push_attribute(("xmlns", NS_XMELD_2611));
        self.attribute.push_to(&mut root);
        writer.write_event(Event::Start(root))?;

        // nachrichtenkopf (type.nachrichtenkopf.privat2b): the message
        // identification is required on the wire; the "alt" structure
        // carries the message type code itself.
        writer.write_event(Event::Start(q("nachrichtenkopf")))?;
        writer.write_event(Event::Start(q("identifikation.nachricht")))?;
        {
            write_code_element(&mut writer, q("nachrichtennummer"), "0600")?;
            write_text_element(
                &mut writer,
                q("erstellungszeitpunkt"),
                &self.ereigniszeitpunkt(),
            )?;
            write_text_element(&mut writer, q("tagesvorgangszaehler"), "1")?;
        }
        writer.write_event(Event::End(BytesEnd::new("identifikation.nachricht")))?;
        writer.write_event(Event::Start(q("absender")))?;
        {
            writer.write_event(Event::Start(q("kunde.bezeichnung")))?;
            writer.write_event(Event::Start(q("NAMEJURISTISCHEPERSON")))?;
            write_text_element(&mut writer, q("name"), &self.input.absender_bezeichnung)?;
            writer.write_event(Event::End(BytesEnd::new("NAMEJURISTISCHEPERSON")))?;
            writer.write_event(Event::End(BytesEnd::new("kunde.bezeichnung")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("absender")))?;
        writer.write_event(Event::Start(q("empfaenger")))?;
        {
            write_text_element(
                &mut writer,
                q("behoerdenkennung"),
                &self.input.empfaenger_behoerdenkennung,
            )?;
        }
        writer.write_event(Event::End(BytesEnd::new("empfaenger")))?;
        writer.write_event(Event::End(BytesEnd::new("nachrichtenkopf")))?;

        write_text_element(
            &mut writer,
            q("zeichennachricht"),
            &self.input.zeichennachricht,
        )?;

        for (position, einzel) in self.input.anfragen.iter().enumerate() {
            let position = position + 1;
            writer.write_event(Event::Start(q("auskunft.anforderung")))?;
            // anfragedaten: the Werbung/Adresshandel declaration is the
            // fachliche Kernpflicht — its absence (code 10) is the one
            // Nichterstellung the Fachanwendung controls.
            writer.write_event(Event::Start(q("anfragedaten")))?;
            write_text_element(
                &mut writer,
                q("zumZweckDerWerbungOderAdresshandel"),
                "false",
            )?;
            writer.write_event(Event::End(BytesEnd::new("anfragedaten")))?;

            writer.write_event(Event::Start(q("datenZurAnfrage")))?;
            writer.write_event(Event::Start(q("name")))?;
            {
                // Suchprofil.Name children are xmeld-qualified (the
                // restriction re-declares them); only their AllgemeinerName
                // payloads are unqualified.
                writer.write_event(Event::Start(q("nachname")))?;
                write_allgemeiner_name(&mut writer, &einzel.nachname)?;
                writer.write_event(Event::End(BytesEnd::new("nachname")))?;
                writer.write_event(Event::Start(q("vornamen")))?;
                write_allgemeiner_name(&mut writer, &einzel.vornamen)?;
                writer.write_event(Event::End(BytesEnd::new("vornamen")))?;
            }
            writer.write_event(Event::End(BytesEnd::new("name")))?;

            let anschrift = &einzel.anschrift;
            writer.write_event(Event::Start(q("anschrift")))?;
            {
                if let Some(ags) = &anschrift.ags {
                    let mut gs = u("gemeindeschluessel");
                    gs.push_attribute(("listVersionID", gemeindeverzeichnis_version()));
                    write_code_element(&mut writer, gs, ags)?;
                }
                if let Some(hn) = &anschrift.hausnummer {
                    write_text_element(&mut writer, u("hausnummer"), hn)?;
                }
                write_text_element(&mut writer, u("postleitzahl"), &anschrift.postleitzahl)?;
                write_text_element(&mut writer, u("strasse"), &anschrift.strasse)?;
                write_text_element(&mut writer, u("wohnort"), &anschrift.wohnort)?;
            }
            writer.write_event(Event::End(BytesEnd::new("anschrift")))?;

            if let Some(geb) = &einzel.geburtsdatum {
                // xida:TeilbekanntesDatumMitUnbekannt = choice(
                //   teilbekanntesDatum → (jahrMonatTag | jahrMonat | jahr),
                //   unbekannt) — the wrapper element is mandatory.
                writer.write_event(Event::Start(q("geburtsdatum")))?;
                if geb.is_unbekannt() {
                    // fixed="true" boolean, unqualified (xida type).
                    write_text_element(&mut writer, u("unbekannt"), "true")?;
                } else {
                    writer.write_event(Event::Start(u("teilbekanntesDatum")))?;
                    let inner = u(geb.granularity.element_name());
                    write_text_element(&mut writer, inner, &geb.value)?;
                    writer.write_event(Event::End(BytesEnd::new("teilbekanntesDatum")))?;
                }
                writer.write_event(Event::End(BytesEnd::new("geburtsdatum")))?;
            }
            writer.write_event(Event::End(BytesEnd::new("datenZurAnfrage")))?;

            writer.write_event(Event::Start(q("technische.einzelidentifikation")))?;
            {
                write_text_element(
                    &mut writer,
                    q("ereigniszeitpunkt"),
                    &self.ereigniszeitpunkt(),
                )?;
                // Mandatory on the wire; deterministic fallback so the
                // caller does not have to invent IDs for trivial flows.
                let zeichen = einzel
                    .zeicheneinzelfall
                    .clone()
                    .unwrap_or_else(|| format!("{}-{position}", self.input.zeichennachricht));
                write_text_element(&mut writer, q("zeicheneinzelfall"), &zeichen)?;
            }
            writer.write_event(Event::End(BytesEnd::new("technische.einzelidentifikation")))?;
            writer.write_event(Event::End(BytesEnd::new("auskunft.anforderung")))?;
        }

        writer.write_event(Event::End(BytesEnd::new(
            "melderegisterauskunfteinfach.anforderung.0600",
        )))?;

        String::from_utf8(buf).map_err(|e| Error::Xml(format!("non-UTF-8 output: {e}")))
    }

    fn ereigniszeitpunkt(&self) -> String {
        self.input
            .ereigniszeitpunkt
            .clone()
            .unwrap_or_else(current_timestamp)
    }
}

/// Write an AllgemeinerName payload: unqualified `name` child (or
/// `nichtVorhanden`). Only the plain-name form is supported for create.
fn write_allgemeiner_name(writer: &mut Writer<&mut Vec<u8>>, value: &str) -> Result<(), Error> {
    let start = u("name");
    write_text_element(writer, start, value)
}

/// Version of the Destatis Gemeindeverzeichnis code list for the
/// `listVersionID` attribute. The value is the GemeindeVZ version the AGS
/// is taken from; the fixture pipeline pins one version — the Fachanwendung
/// is responsible for its own DVDV/GemeindeVZ vintage, so this is a
/// constant, not configuration, until a real need appears.
fn gemeindeverzeichnis_version() -> &'static str {
    "20260701"
}

/// Current timestamp in xs:dateTime form with offset, second resolution.
fn current_timestamp() -> String {
    // std has no formatter for offsets; hand-rolling the minimal valid
    // xs:dateTime (UTC, Z suffix) avoids a chrono dependency for one line.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days algorithm (Howard Hinnant) — correct for all dates
    // after 1970-03-01 and before 1970-03-01 needs the +719468 offset path,
    // which is included.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}
