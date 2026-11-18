//! Create Nachricht 0600 („Anfrage zur einfachen Melderegisterauskunft“)
//! from structured input.
//!
//! The JSON contract ([`AnfrageInput`]) is what the Fachanwendung hands
//! over; the XSD gate (crate::validate) proves the emitted XML against the
//! official XMeld 26.11 schemas. The full Suchprofil is supported: name +
//! Anschrift or name + two further search criteria (the fachlich Suchregel,
//! enforced fail-fast in [`Anfrage::build`]), partial birth dates,
//! Geschlecht/Familienstand, Geburts- und Sterbeort, Sterbetag,
//! Einzugs-/Auszugsdatum, Ordnens-/Künstlername, gesetzlicher Vertreter,
//! Partnername, gewerblicher Zweck and steuerungsinformationen. Known
//! limitation: `wiederholanfrage` referencing previous orders is not
//! exposed yet.

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
///
/// The fachlich Suchregel (datenZurAnfrage documentation, XMeld 26.11):
/// „Die Namensangaben sind verpflichtend, von den sonstigen Suchkriterien
/// sind mindestens eine Anschrift, oder zwei der weiteren zulässigen Daten
/// anzugeben.“ — enforced fail-fast in [`Anfrage::build`]. Which criteria
/// count as „zulässige Daten“ is a Schematron-level rule rmeld cannot read
/// from the XSD; it conservatively counts every supplied optional criterion
/// and lets the Behörde have the final word (a rejected Suchprofil arrives
/// as Nichterstellung.grund 08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EinzelanfrageInput {
    /// Correlation ID for this Einzelanfrage (zeicheneinzelfall), echoed
    /// per Auskunft position. Optional in the JSON — but **mandatory on
    /// the wire** (technische.einzelidentifikation), so when absent rmeld
    /// emits `{zeichennachricht}-{1-based position}` deterministically.
    #[serde(default)]
    pub zeicheneinzelfall: Option<String>,
    /// Familienname (Suchprofil.name/nachname). Required on the wire.
    pub nachname: String,
    /// Vornamen (Suchprofil.name/vornamen). Required on the wire.
    pub vornamen: String,
    /// Geburtsdatum, fully or partially known: `YYYY-MM-DD`, `YYYY-MM`,
    /// `YYYY`, or `"unbekannt"`.
    #[serde(default)]
    pub geburtsdatum: Option<PartialDatum>,
    /// Anschrift des Suchprofils (Inland). Optional — one Anschrift or two
    /// further criteria satisfy the Suchregel.
    #[serde(default)]
    pub anschrift: Option<AnschriftInput>,
    /// Ordensname (datatypeC).
    #[serde(default)]
    pub ordensname: Option<String>,
    /// Künstlername (datatypeC).
    #[serde(default)]
    pub kuenstlername: Option<String>,
    /// Geburtsort und -staat (ohne Datum) — für Personen ohne bekannten
    /// Tag. `staat_code` carries a Code.Gebiet value (Destatis
    /// Staatsgebiete); the `listVersionID` attribute defaults to
    /// [`gemeindeverzeichnis_version`]'s sibling constant.
    #[serde(default)]
    pub geburtsort: Option<OrtMitStaat>,
    /// Geschlecht (Code `m`/`w`/`x`/`d`, XInneres-Codeliste geschlecht v1)
    /// oder Familienstand (Codes `EA`, `GS`, `LA`, `LD`, `LE`, `LP`, `LV`,
    /// `NB`, `VH`, `VW`). Both together are not zulässig — the wire is a
    /// choice and the input is one enum.
    #[serde(default)]
    pub geschlecht_oder_familienstand: Option<GeschlechtOderFamilienstand>,
    /// Gesetzlicher Vertreter — natürliche Person (Familienname +
    /// Vornamen) or juristische Person (Name).
    #[serde(default)]
    pub gesetzlicher_vertreter: Option<GesetzlicherVertreter>,
    /// Einzugsdatum (`YYYY-MM-DD`).
    #[serde(default)]
    pub einzugsdatum: Option<Datum>,
    /// Auszugsdatum (`YYYY-MM-DD`).
    #[serde(default)]
    pub auszugsdatum: Option<Datum>,
    /// Beginn der letzten Ehe/Lebenspartnerschaft, Ort und (optional)
    /// Staat; Datum ohne „unbekannt“ (xida:TeilbekanntesDatum).
    #[serde(default)]
    pub familienstand_beginn: Option<FamilienstandBeginn>,
    /// Name des Partners/der Partnerin (alle Felder optional auf dem Draht).
    #[serde(default)]
    pub partner: Option<Partner>,
    /// Sterbetag (`YYYY-MM-DD`, via type.Sterbetag).
    #[serde(default)]
    pub sterbetag: Option<Datum>,
    /// Sterbeort und -staat (ohne Datum).
    #[serde(default)]
    pub sterbeort: Option<OrtMitStaat>,
    /// steuerungsinformationen/option.auskunft codes: `01` (Aussteuerung
    /// zur manuellen Nachbearbeitung) and/or `04` (urschriftlicher Druck).
    #[serde(default)]
    pub option_auskunft: Vec<String>,
    /// steuerungsinformationen/verzichtAufMitteilung (fixed `true` on the
    /// wire when set).
    #[serde(default)]
    pub verzicht_auf_mitteilung: bool,
    /// steuerungsinformationen/wiederholanfrage — reference to the order
    /// this request repeats, for Behörden that could not finish in one
    /// pass. Take `auftragsnummer` and `auftragspositionsnummer` from the
    /// previous [`crate::Antwort`] (message-level field and position field
    /// of the same names). At least one reference is required; both
    /// together when the previous order had multiple positions.
    #[serde(default)]
    pub wiederholanfrage: Option<Wiederholanfrage>,
    /// anfragedaten/gewerblicherZweck — codes 1–10 (Adressabgleich,
    /// Werbung, Adresshandel, …) und/oder Freitext; `geschaeftszeichen` is
    /// mandatory when the Zweck is supplied (schema requirement).
    #[serde(default)]
    pub gewerblicher_zweck: Option<GewerblicherZweck>,
}

/// Wiederholanfrage reference (type.wiederholanfrage): the order being
/// asked about again. Both fields optional on the wire; rmeld requires at
/// least one (an empty reference is meaningless — fail fast).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Wiederholanfrage {
    /// auftragsnummer.alteanfrage — the previous order number
    /// (Antwort.auftragsnummer).
    #[serde(default)]
    pub auftragsnummer_alteanfrage: Option<String>,
    /// auftragspositionsnummer.alteanfrage — the previous position
    /// (AuskunftPosition.auftragspositionsnummer).
    #[serde(default)]
    pub auftragspositionsnummer_alteanfrage: Option<String>,
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

/// Ort mit (optionalem) Staat — geburtsortUndStaat / sterbeortUndStaat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrtMitStaat {
    /// Der Ort (datatypeC). Required on the wire.
    pub ort: String,
    /// Code.Gebiet value (Destatis Staatsgebiete, e.g. „ITA“ for Italien);
    /// `listVersionID` defaults to [`gebiet_version`].
    #[serde(default)]
    pub staat_code: Option<String>,
}

/// Geschlecht oder Familienstand — the wire is a choice, the input is one
/// enum. Codes are kept raw: `m`/`w`/`x`/`d` (geschlecht, Codeliste v1)
/// or the DSMeld familienstand codes (`EA`, `GS`, `LA`, `LD`, `LE`, `LP`,
/// `LV`, `NB`, `VH`, `VW`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeschlechtOderFamilienstand {
    /// `geschlecht` — Code.Geschlecht wrapper.
    Geschlecht(String),
    /// `familienstand` — Code.Familienstand wrapper.
    Familienstand(String),
}

/// Gesetzlicher Vertreter — choice auf dem Draht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GesetzlicherVertreter {
    /// Natürliche Person: Familienname + Vornamen.
    Natuerlich {
        /// Familienname des Vertreters.
        familienname: String,
        /// Vornamen des Vertreters.
        vornamen: String,
    },
    /// Juristische Person: der Name als Zeichenkette.
    Juristisch(String),
}

/// Beginn der letzten Ehe/Lebenspartnerschaft, mit Ort. Das Datum ist ein
/// TeilbekanntesDatum (JJJJ-MM-TT, JJJJ-MM, JJJJ) — „unbekannt“ is
/// rejected here (the wire type has no unbekannt arm).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilienstandBeginn {
    /// Datum des Beginns (teilbekannt).
    pub datum: PartialDatum,
    /// ort.letzte.ehe.oder.lp (datatypeC). Required on the wire.
    pub ort: String,
    /// ortstaat.letzte.ehe.oder.lp (Code.Gebiet).
    #[serde(default)]
    pub staat_code: Option<String>,
    /// datumBeginnVorausgegangeneLP (`YYYY-MM-DD`).
    #[serde(default)]
    pub datum_vorausgegangene_lp: Option<Datum>,
}

/// Partnername — alle Felder optional auf dem Draht
/// (type.NameNatuerlichePerson.Partner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Partner {
    /// Familienname des Partners (FamiliennameUnstrukturiert/nachname).
    #[serde(default)]
    pub familienname: Option<String>,
    /// Geburtsname des Partners (AllgemeinerName).
    #[serde(default)]
    pub geburtsname: Option<String>,
    /// Vornamen des Partners (AllgemeinerName).
    #[serde(default)]
    pub vornamen: Option<String>,
    /// Doktorgrad des Partners.
    #[serde(default)]
    pub doktorgrad: Option<String>,
}

/// Gewerblicher Zweck nach § 44 Abs. 1 Satz 2 BMG — codes 1–10 der
/// Schlüsseltabelle melderegisterauskunft.gewerblicher.zweck (1
/// Adressabgleich, 2 Adressermittlung, 3 Adressabgleich für Dritte,
/// 4 Aktualisierung eigener Bestandsdaten, 5 Adresshistorisierung,
/// 6 Forderungsmanagement, 7 Bonitätsrisikoprüfungen, 8 Werbung,
/// 9 Adresshandel, 10 Markt-, Meinungs- oder Sozialforschung) und/oder
/// Freitext; `geschaeftszeichen` is schema-mandatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GewerblicherZweck {
    /// Codes 1–10 aus der Schlüsseltabelle.
    #[serde(default)]
    pub codes: Vec<String>,
    /// Konkreter Zweck als Freitext, falls kein Code passt.
    #[serde(default)]
    pub freitext: Option<String>,
    /// geschaeftszeichen — required when the Zweck is supplied.
    pub geschaeftszeichen: String,
}

/// A full calendar date (`YYYY-MM-DD`), validated at deserialization —
/// the wire type is a plain `xs:date` with a pattern (xida:Tagesdatum).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Datum(String);

impl Datum {
    /// The validated date string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Datum {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let s = s.trim().to_string();
        let parts: Vec<&str> = s.split('-').collect();
        let ok = parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            && parts[0].len() == 4
            && parts[1].len() == 2
            && parts[2].len() == 2;
        if ok {
            Ok(Datum(s))
        } else {
            Err(format!("date must be YYYY-MM-DD, got {s:?}"))
        }
    }
}

impl std::str::FromStr for Datum {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Datum::try_from(s.to_string())
    }
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
    /// error, never a warning. The big one is the Suchregel („mindestens
    /// eine Anschrift, oder zwei der weiteren zulässigen Daten“); see the
    /// [EinzelanfrageInput] docs for the conservative reading of
    /// „zulässige Daten“.
    pub fn build(input: AnfrageInput) -> Result<Anfrage, Error> {
        if input.anfragen.is_empty() {
            return Err(Error::InvalidRequest(
                "at least one Einzelanfrage is required (auskunft.anforderung maxOccurs=unbounded, minOccurs=1)",
            ));
        }
        for anfrage in &input.anfragen {
            validate_suchregel(anfrage)?;
            if let Some(beginn) = &anfrage.familienstand_beginn
                && beginn.datum.is_unbekannt()
            {
                return Err(Error::InvalidRequest(
                    "familienstand_beginn.datum must not be \"unbekannt\" (xida:TeilbekanntesDatum has no unbekannt arm)",
                ));
            }
            if let Some(w) = &anfrage.wiederholanfrage
                && w.auftragsnummer_alteanfrage
                    .as_deref()
                    .unwrap_or("")
                    .is_empty()
                && w.auftragspositionsnummer_alteanfrage
                    .as_deref()
                    .unwrap_or("")
                    .is_empty()
            {
                return Err(Error::InvalidRequest(
                    "wiederholanfrage requires at least one of auftragsnummer_alteanfrage / auftragspositionsnummer_alteanfrage — an empty reference names no order",
                ));
            }
            if let Some(zweck) = &anfrage.gewerblicher_zweck {
                if zweck.codes.is_empty() && zweck.freitext.as_deref().unwrap_or("").is_empty() {
                    return Err(Error::InvalidRequest(
                        "gewerblicher_zweck requires at least one code or a freitext",
                    ));
                }
                if zweck.geschaeftszeichen.trim().is_empty() {
                    return Err(Error::InvalidRequest(
                        "gewerblicher_zweck.geschaeftszeichen is mandatory (type.GewerblicherZweck)",
                    ));
                }
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

            // anfragedaten — sequence order matters: gewerblicherZweck?,
            // zeicheneinzelfall?, zumZweckDerWerbungOderAdresshandel.
            // The Werbung/Adresshandel declaration is the fachliche
            // Kernpflicht — its absence (code 10) is the one Nichterstellung
            // the Fachanwendung controls.
            writer.write_event(Event::Start(q("anfragedaten")))?;
            if let Some(zweck) = &einzel.gewerblicher_zweck {
                writer.write_event(Event::Start(q("gewerblicherZweck")))?;
                writer.write_event(Event::Start(q("angabeDesZwecks")))?;
                for code in &zweck.codes {
                    let mut wrapper = q("code");
                    wrapper.push_attribute(("listVersionID", gewerblicher_zweck_version()));
                    write_code_element(&mut writer, wrapper, code)?;
                }
                if let Some(freitext) = &zweck.freitext {
                    write_text_element(&mut writer, q("freitext"), freitext)?;
                }
                writer.write_event(Event::End(BytesEnd::new("angabeDesZwecks")))?;
                write_text_element(
                    &mut writer,
                    q("geschaeftszeichen"),
                    &zweck.geschaeftszeichen,
                )?;
                writer.write_event(Event::End(BytesEnd::new("gewerblicherZweck")))?;
            }
            if let Some(z) = &einzel.zeicheneinzelfall {
                write_text_element(&mut writer, q("zeicheneinzelfall"), z)?;
            }
            write_text_element(
                &mut writer,
                q("zumZweckDerWerbungOderAdresshandel"),
                "false",
            )?;
            writer.write_event(Event::End(BytesEnd::new("anfragedaten")))?;

            // datenZurAnfrage — the full Suchprofil, in schema sequence
            // order. The official reference messages are the form
            // convention ground truth (qualified wrappers, unqualified
            // leaf data).
            writer.write_event(Event::Start(q("datenZurAnfrage")))?;
            writer.write_event(Event::Start(q("name")))?;
            {
                writer.write_event(Event::Start(q("nachname")))?;
                write_allgemeiner_name(&mut writer, &einzel.nachname)?;
                writer.write_event(Event::End(BytesEnd::new("nachname")))?;
                writer.write_event(Event::Start(q("vornamen")))?;
                write_allgemeiner_name(&mut writer, &einzel.vornamen)?;
                writer.write_event(Event::End(BytesEnd::new("vornamen")))?;
            }
            writer.write_event(Event::End(BytesEnd::new("name")))?;

            if let Some(anschrift) = &einzel.anschrift {
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
            }

            if let Some(o) = &einzel.ordensname {
                write_text_element(&mut writer, q("ordensname"), o)?;
            }
            if let Some(k) = &einzel.kuenstlername {
                write_text_element(&mut writer, q("kuenstlername"), k)?;
            }

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

            if let Some(ort) = &einzel.geburtsort {
                write_ort_mit_staat(&mut writer, "geburtsortUndStaat", "geburtsort", ort)?;
            }

            if let Some(gof) = &einzel.geschlecht_oder_familienstand {
                writer.write_event(Event::Start(q("geschlechtOderFamilienstand")))?;
                match gof {
                    GeschlechtOderFamilienstand::Geschlecht(code) => {
                        let mut wrapper = q("geschlecht");
                        wrapper.push_attribute(("listVersionID", geschlecht_version()));
                        write_code_element(&mut writer, wrapper, code)?;
                    }
                    GeschlechtOderFamilienstand::Familienstand(code) => {
                        write_code_element(&mut writer, q("familienstand"), code)?;
                    }
                }
                writer.write_event(Event::End(BytesEnd::new("geschlechtOderFamilienstand")))?;
            }

            if let Some(v) = &einzel.gesetzlicher_vertreter {
                writer.write_event(Event::Start(q("gesetzlicherVertreter.name")))?;
                match v {
                    GesetzlicherVertreter::Natuerlich {
                        familienname,
                        vornamen,
                    } => {
                        writer.write_event(Event::Start(q("nameNatuerlichePerson")))?;
                        writer.write_event(Event::Start(q("familienname.unstrukturiert")))?;
                        writer.write_event(Event::Start(q("nachname")))?;
                        write_allgemeiner_name(&mut writer, familienname)?;
                        writer.write_event(Event::End(BytesEnd::new("nachname")))?;
                        writer.write_event(Event::End(BytesEnd::new(
                            "familienname.unstrukturiert",
                        )))?;
                        writer.write_event(Event::Start(q("vornamen")))?;
                        write_allgemeiner_name(&mut writer, vornamen)?;
                        writer.write_event(Event::End(BytesEnd::new("vornamen")))?;
                        writer.write_event(Event::End(BytesEnd::new("nameNatuerlichePerson")))?;
                    }
                    GesetzlicherVertreter::Juristisch(name) => {
                        write_text_element(&mut writer, q("nameJuristischePerson"), name)?;
                    }
                }
                writer.write_event(Event::End(BytesEnd::new("gesetzlicherVertreter.name")))?;
            }

            if let Some(d) = &einzel.einzugsdatum {
                write_text_element(&mut writer, q("einzugsdatum"), d.as_str())?;
            }
            if let Some(d) = &einzel.auszugsdatum {
                write_text_element(&mut writer, q("auszugsdatum"), d.as_str())?;
            }

            if let Some(beginn) = &einzel.familienstand_beginn {
                writer.write_event(Event::Start(q("familienstandBeginnUndOrt")))?;
                // TeilbekanntesDatum (no unbekannt arm — build() rejects it).
                // The wrapper element is xmeld-qualified (the restriction
                // re-declares it); only its xida children are unqualified.
                writer.write_event(Event::Start(q("datumBeginn")))?;
                let inner = u(beginn.datum.granularity.element_name());
                write_text_element(&mut writer, inner, &beginn.datum.value)?;
                writer.write_event(Event::End(BytesEnd::new("datumBeginn")))?;
                write_text_element(&mut writer, q("ort.letzte.ehe.oder.lp"), &beginn.ort)?;
                if let Some(staat) = &beginn.staat_code {
                    let mut wrapper = q("ortstaat.letzte.ehe.oder.lp");
                    wrapper.push_attribute(("listVersionID", gebiet_version()));
                    write_code_element(&mut writer, wrapper, staat)?;
                }
                if let Some(d) = &beginn.datum_vorausgegangene_lp {
                    write_text_element(&mut writer, q("datumBeginnVorausgegangeneLP"), d.as_str())?;
                }
                writer.write_event(Event::End(BytesEnd::new("familienstandBeginnUndOrt")))?;
            }

            if let Some(p) = &einzel.partner {
                writer.write_event(Event::Start(q("partner.name")))?;
                if let Some(f) = &p.familienname {
                    writer.write_event(Event::Start(q("familienname.unstrukturiert")))?;
                    writer.write_event(Event::Start(q("nachname")))?;
                    write_allgemeiner_name(&mut writer, f)?;
                    writer.write_event(Event::End(BytesEnd::new("nachname")))?;
                    writer.write_event(Event::End(BytesEnd::new("familienname.unstrukturiert")))?;
                }
                if let Some(g) = &p.geburtsname {
                    writer.write_event(Event::Start(q("geburtsname.unstrukturiert")))?;
                    write_allgemeiner_name(&mut writer, g)?;
                    writer.write_event(Event::End(BytesEnd::new("geburtsname.unstrukturiert")))?;
                }
                if let Some(v) = &p.vornamen {
                    writer.write_event(Event::Start(q("vornamen")))?;
                    write_allgemeiner_name(&mut writer, v)?;
                    writer.write_event(Event::End(BytesEnd::new("vornamen")))?;
                }
                if let Some(d) = &p.doktorgrad {
                    write_text_element(&mut writer, q("doktorgrad"), d)?;
                }
                writer.write_event(Event::End(BytesEnd::new("partner.name")))?;
            }

            if let Some(d) = &einzel.sterbetag {
                // type.Sterbetag = wrapper element + inner sterbetag date.
                writer.write_event(Event::Start(q("sterbetag")))?;
                write_text_element(&mut writer, q("sterbetag"), d.as_str())?;
                writer.write_event(Event::End(BytesEnd::new("sterbetag")))?;
            }

            if let Some(ort) = &einzel.sterbeort {
                write_ort_mit_staat(&mut writer, "sterbeortUndStaat", "sterbeort", ort)?;
            }

            writer.write_event(Event::End(BytesEnd::new("datenZurAnfrage")))?;

            // steuerungsinformationen — sequence: option.auskunft*,
            // wiederholanfrage?, verzichtAufMitteilung?.
            if !einzel.option_auskunft.is_empty()
                || einzel.verzicht_auf_mitteilung
                || einzel.wiederholanfrage.is_some()
            {
                writer.write_event(Event::Start(q("steuerungsinformationen")))?;
                for option in &einzel.option_auskunft {
                    write_code_element(&mut writer, q("option.auskunft"), option)?;
                }
                if let Some(w) = &einzel.wiederholanfrage {
                    writer.write_event(Event::Start(q("wiederholanfrage")))?;
                    if let Some(n) = &w.auftragsnummer_alteanfrage {
                        write_text_element(&mut writer, q("auftragsnummer.alteanfrage"), n)?;
                    }
                    if let Some(p) = &w.auftragspositionsnummer_alteanfrage {
                        write_text_element(
                            &mut writer,
                            q("auftragspositionsnummer.alteanfrage"),
                            p,
                        )?;
                    }
                    writer.write_event(Event::End(BytesEnd::new("wiederholanfrage")))?;
                }
                if einzel.verzicht_auf_mitteilung {
                    write_text_element(&mut writer, q("verzichtAufMitteilung"), "true")?;
                }
                writer.write_event(Event::End(BytesEnd::new("steuerungsinformationen")))?;
            }

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

/// Write `geburtsortUndStaat` / `sterbeortUndStaat`:
/// qualified wrapper, qualified ort text, optional Code.Gebiet staat.
fn write_ort_mit_staat(
    writer: &mut Writer<&mut Vec<u8>>,
    wrapper: &'static str,
    ort_element: &'static str,
    ort: &OrtMitStaat,
) -> Result<(), Error> {
    writer.write_event(Event::Start(q(wrapper)))?;
    write_text_element(writer, q(ort_element), &ort.ort)?;
    if let Some(staat) = &ort.staat_code {
        let element = if wrapper == "sterbeortUndStaat" {
            "sterbeortStaat"
        } else {
            "geburtsortStaat"
        };
        let mut code = q(element);
        code.push_attribute(("listVersionID", gebiet_version()));
        write_code_element(writer, code, staat)?;
    }
    writer.write_event(Event::End(BytesEnd::new(wrapper)))?;
    Ok(())
}

/// The fachlich Suchregel: name (always present) + one Anschrift, or name
/// + two further criteria.
///
/// The set of „zulässige Daten“ is Schematron territory; rmeld
/// conservatively counts every supplied optional criterion (superset) and
/// documents the limitation — a Behörde-side rejection arrives as
/// Nichterstellung.grund 08 and classifies as Rejected, which is the
/// contract, not a crash.
fn validate_suchregel(einzel: &EinzelanfrageInput) -> Result<(), Error> {
    if let Some(anschrift) = &einzel.anschrift {
        if anschrift.postleitzahl.is_empty()
            || anschrift.strasse.is_empty()
            || anschrift.wohnort.is_empty()
        {
            return Err(Error::InvalidRequest(
                "Anschrift requires postleitzahl, strasse and wohnort",
            ));
        }
        return Ok(());
    }
    let weitere = [
        einzel.geburtsdatum.is_some(),
        einzel.geburtsort.is_some(),
        einzel.geschlecht_oder_familienstand.is_some(),
        einzel.gesetzlicher_vertreter.is_some(),
        einzel.einzugsdatum.is_some(),
        einzel.auszugsdatum.is_some(),
        einzel.familienstand_beginn.is_some(),
        einzel.partner.is_some(),
        einzel.sterbetag.is_some(),
        einzel.sterbeort.is_some(),
        einzel.ordensname.is_some(),
        einzel.kuenstlername.is_some(),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    if weitere < 2 {
        return Err(Error::InvalidRequest(
            "Suchregel verletzt: ohne Anschrift sind mindestens zwei weitere Suchkriterien erforderlich („mindestens eine Anschrift, oder zwei der weiteren zulässigen Daten“)",
        ));
    }
    Ok(())
}

/// Version of the Destatis Gemeindeverzeichnis code list for the
/// `listVersionID` attribute. The value is the GemeindeVZ version the AGS
/// is taken from; the fixture pipeline pins one version — the Fachanwendung
/// is responsible for its own DVDV/GemeindeVZ vintage, so this is a
/// constant, not configuration, until a real need appears.
fn gemeindeverzeichnis_version() -> &'static str {
    "20260701"
}

/// Version of the XInneres geschlecht code list
/// (urn:xoev-de:xinneres:codeliste:geschlecht, v1: m/w/x/d), fetched from
/// XRepository. Same fixture-constant rationale as above.
fn geschlecht_version() -> &'static str {
    "1"
}

/// Version of the Destatis Staatsgebiete code list
/// (urn:de:bund:destatis:bevoelkerungsstatistik:schluessel:staatsgebiete).
/// Open code list — the schema pins no version; the constant keeps the
/// required attribute populated until a real vintage exists.
fn gebiet_version() -> &'static str {
    "2"
}

/// Version of the gewerblicher Zweck code list
/// (urn:de:xmeld:schluesseltabelle:melderegisterauskunft.gewerblicher.
/// zweck, v1: codes 1–10).
fn gewerblicher_zweck_version() -> &'static str {
    "1"
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
