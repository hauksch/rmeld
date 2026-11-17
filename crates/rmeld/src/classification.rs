//! The classification taxonomy for Melderegisterauskunft responses.
//!
//! Derived from the vendored XMeld 26.11 schemas, not from memory:
//! every `code` value and its official text comes from the enumerations in
//! `schema/xmeld/xmeld-codes.xsd` (urn:de:xmeld:schluesseltabelle:*), and
//! the message kinds from the global element declarations in
//! `schema/xmeld/xmeld-nachrichten-melderegisterauskunft.xsd` and
//! `…-administration.xsd`. See docs/STANDARD-NOTES.md for the mapping.

/// The fachlich verdict for one Anfrage, one Auskunft position or a whole
/// message. This is the taxonomy the Fachanwendung switches on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// „Der Betroffene wurde im Rahmen der automatisierten Suche im
    /// Melderegister eindeutig identifiziert. Die Daten werden übermittelt.“
    /// (ergebnisstatus 01) — carries the person data the Behörde sent.
    Fulfilled(Ergebnis),
    /// „Eine Auskunft kann aus tatsächlichen oder rechtlichen Gründen nicht
    /// oder derzeit nicht erteilt werden.“ (ergebnisstatus 05) — the neutral
    /// answer, including Auskunftssperren (§ 51, § 52 BMG). No person data.
    Neutral,
    /// Fachliche Rückweisung: either a per-Anfrage Nichterstellung
    /// (rueckweisung element, Schlüsseltabelle
    /// melderegisterauskunft.nichterstellung.grund) or a message-level
    /// Antwortstatus rejection (03 fehlende Berechtigung, 04 Gebührenregelung).
    Rejected(Rejection),
    /// Technische Ablehnung: a message-level Antwortstatus technical fault
    /// (01 Anfrage nicht lesbar, 05 Leistung nicht unterstützt) or an
    /// administration.returntosender.0900/0910 RTS message.
    TechnicalFault(TechnicalFault),
}

/// Person data from a fulfilled answer (ergebnis element).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ergebnis {
    /// Raw ergebnisstatus code as sent („01“ for fulfilled answers; kept raw
    /// so unknown future codes still round-trip).
    pub ergebnisstatus: String,
    /// Raw Zusatzinformation codes (e.g. „01“ andere Adresse, „04“ verstorben).
    pub zusatzinformationen: Vec<String>,
    /// Name of the identified person, if given.
    pub namen: Option<PersonName>,
    /// Current registered addresses (anschrift.aktuell, Inland form).
    pub anschriften: Vec<Anschrift>,
}

/// Name block of a fulfilled answer (type.Melderegisterauskunft.Name).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersonName {
    /// familienname.unstrukturiert/nachname.
    pub familienname: Option<String>,
    /// vornamen/vornamen.
    pub vornamen: Option<String>,
    /// gebraeuchlicherVorname.
    pub gebraeuchlicher_vorname: Option<String>,
    /// doktorgrad.
    pub doktorgrad: Option<String>,
}

/// Registered address of a fulfilled answer (Inland form of
/// type.AnschriftMelderecht). Only the fields a Fachanwendung acts on are
/// surfaced; the XSD gate keeps the full wire fidelity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Anschrift {
    /// Amtlicher Gemeindeschlüssel (gemeindeschluessel/code).
    pub ags: Option<String>,
    /// hausnummer.
    pub hausnummer: Option<String>,
    /// postleitzahl.
    pub postleitzahl: Option<String>,
    /// strasse.
    pub strasse: Option<String>,
    /// wohnort.
    pub wohnort: Option<String>,
}

/// Source of a fachlich rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionSource {
    /// rueckweisung element inside one Auskunft position.
    NichterstellungEinzelanfrage,
    /// Message-level antwortstatus (codes 03, 04).
    Antwortstatus,
}

/// A fachlich rejection, with the raw code for round-tripping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// Where the rejection was expressed (position-level or message-level).
    pub quelle: RejectionSource,
    /// Raw code (Nichterstellung.grund 01–10 or Antwortstatus 03/04).
    pub code: String,
}

/// Source of a technical fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultSource {
    /// Message-level antwortstatus (codes 01, 05).
    Antwortstatus,
    /// administration.returntosender.0900/0910 message.
    ReturnToSender,
}

/// A technical fault, with the raw RTS or Antwortstatus code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnicalFault {
    /// Where the fault was expressed (Antwortstatus or RTS message).
    pub quelle: FaultSource,
    /// Raw code (Antwortstatus 01/05 or RTS grund like „T099“, „X010“).
    pub code: String,
}

impl Classification {
    /// The official German text of the underlying code, straight from the
    /// Schlüsseltabellen as vendored in schema/xmeld/xmeld-codes.xsd.
    /// Unknown codes render as the raw code — the code lists are
    /// versioned by the standard and may grow.
    pub fn text(&self) -> String {
        match self {
            Classification::Fulfilled(e) => match e.ergebnisstatus.as_str() {
                "01" => "Der Betroffene wurde im Rahmen der automatisierten Suche im Melderegister eindeutig identifiziert. Die Daten werden übermittelt.".to_string(),
                other => format!("ergebnisstatus {other}"),
            },
            Classification::Neutral => "Eine Auskunft kann aus tatsächlichen oder rechtlichen Gründen nicht oder derzeit nicht erteilt werden.".to_string(),
            Classification::Rejected(r) => rejection_text(r),
            Classification::TechnicalFault(f) => fault_text(f),
        }
    }
}

/// Official Nichterstellung.grund texts (urn:de:xmeld:schluesseltabelle:
/// melderegisterauskunft.nichterstellung.grund, version 5).
fn rejection_text(r: &Rejection) -> String {
    match (r.quelle, r.code.as_str()) {
        (RejectionSource::Antwortstatus, "03") => "Berechtigung für diese Anfrage fehlt.".to_string(),
        (RejectionSource::Antwortstatus, "04") => "Gebührenregelung nicht positiv abgeschlossen.".to_string(),
        (RejectionSource::NichterstellungEinzelanfrage, code) => match code {
            "01" => "Die Angaben zum gewerblichen Zweck in der Anfrage sind fehlerhaft.",
            "04" => "Die zu protokollierenden Daten sind nicht vollständig.",
            "05" => "Der Gemeindename, der Amtliche Gemeindeschlüssel oder die Straße innerhalb der Gemeinde existiert nicht.",
            "06" => "Die Berechtigung für diese Einzelanfrage fehlt oder ist nicht ausreichend.",
            "07" => "Sonstiger technischer Fehler dieser Einzelanfrage (z. B. korrupte Daten)",
            "08" => "Das Suchprofil dieser Einzelanfrage ist ungültig oder wird nicht unterstützt.",
            "09" => "Die gemeindeübergreifende Suche wurde abgebrochen. Die in der Steuerungsinformation angegebenen Restriktionen sind eingetreten.",
            "10" => "In der Anfrage fehlt die Erklärung, dass die Daten nicht zum Zweck der Werbung oder des Adresshandels verwendet werden. Es darf daher keine Auskunft erfolgen.",
            _ => return format!("nichterstellung.grund {code}"),
        }
        .to_string(),
        (RejectionSource::Antwortstatus, code) => format!("antwortstatus {code}"),
    }
}

/// Official Antwortstatus texts (urn:de:xmeld:schluesseltabelle:
/// antwortstatus, version 2) and RTS prefix semantics
/// (urn:de:xmeld:schluesseltabelle:ruecksendung.einer.nachricht, v5:
/// S* nicht spezifikationskonform, T* Zertifikat/Transport, X* XML,
/// V* Versionsproblem).
fn fault_text(f: &TechnicalFault) -> String {
    match (f.quelle, f.code.as_str()) {
        (FaultSource::Antwortstatus, "01") => {
            "Anfrage kann aus technischen Gründen nicht gelesen werden (Anfragefehler).".to_string()
        }
        (FaultSource::Antwortstatus, "05") => {
            "Angeforderte Leistung wird technisch derzeit nicht unterstützt.".to_string()
        }
        (FaultSource::Antwortstatus, code) => format!("antwortstatus {code}"),
        (FaultSource::ReturnToSender, code) => match code.as_bytes().first() {
            Some(b'S') => format!("RTS {code}: nicht spezifikationskonform"),
            Some(b'T') => format!("RTS {code}: Zertifikats- oder Transportproblem"),
            Some(b'X') => format!("RTS {code}: XML- oder Schlüsseltabellenproblem"),
            Some(b'V') => format!("RTS {code}: Versionsproblem"),
            _ => format!("RTS {code}"),
        },
    }
}
