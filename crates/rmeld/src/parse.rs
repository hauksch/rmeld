//! Parse xMeld responses into typed results: Nachricht 0601 („Antwort zur
//! einfachen Melderegisterauskunft“) and the technical-fault vehicles
//! administration.returntosender.0900/0910; parsing 0600 Anfragen back is
//! supported for round-trip verification.
//!
//! Parsing is namespace-aware via quick-xml's `NsReader`: the wire mixes
//! qualified xmeld elements with unqualified leaf data (see
//! [`crate::wire`]), and only the known release namespaces are accepted on
//! the root. Unknown structure fails loudly ([`crate::Error::Malformed`])
//! rather than half-working.

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

use crate::classification::{
    Anschrift, Classification, Ergebnis, FaultSource, PersonName, Rejection, RejectionSource,
    TechnicalFault,
};
use crate::error::Error;
use crate::wire::KNOWN_NAMESPACES;

/// A parsed xMeld response message.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Nachricht 0600, parsed back for round-trip verification.
    Anfrage0600(AnfrageEcho),
    /// Nachricht 0601 — the fachlich answer.
    Antwort0601(Antwort),
    /// administration.returntosender.0900/0910 — technical rejection.
    ReturnToSender(Rts),
}

impl Message {
    /// Parse a byte-exact xMeld message. The root namespace must be one of
    /// the known release namespaces; the root element decides the variant.
    /// An `<XTA>` envelope root (the transport-file convention, any
    /// namespace, case-insensitive like rosci's sniff) is transparent:
    /// parsing descends to the first element inside it.
    pub fn parse(bytes: &[u8]) -> Result<Message, Error> {
        let mut reader = NsReader::from_reader(bytes);
        reader.config_mut().trim_text(true);
        let mut in_envelope = false;
        loop {
            let (resolved, event) = reader
                .read_resolved_event()
                .map_err(|e| Error::Malformed(format!("xml error: {e}")))?;
            let ns: String = match &resolved {
                ResolveResult::Bound(uri) => std::str::from_utf8(uri.as_ref())
                    .map_err(|e| Error::Malformed(format!("namespace not UTF-8: {e}")))?
                    .to_string(),
                ResolveResult::Unbound => String::new(),
                ResolveResult::Unknown(prefix) => {
                    return Err(Error::Malformed(format!(
                        "unknown namespace prefix binding: {prefix:?}"
                    )));
                }
            };
            match event {
                Event::Start(start) => {
                    let root = local_name(&start);
                    if in_envelope {
                        // First element inside the envelope: the message.
                        assert_known_namespace(&ns, &root)?;
                        return dispatch_root(&mut reader, root, ns);
                    }
                    if root.eq_ignore_ascii_case("XTA") {
                        in_envelope = true;
                        continue;
                    }
                    assert_known_namespace(&ns, &root)?;
                    return dispatch_root(&mut reader, root, ns);
                }
                Event::Empty(start) => {
                    let root = local_name(&start);
                    if in_envelope {
                        // The envelope's only child is self-closing: no
                        // fachlich content — but the name still decides
                        // between "known but empty" and "unknown".
                        assert_known_namespace(&ns, &root)?;
                        if matches!(
                            root.as_str(),
                            "melderegisterauskunfteinfach.anforderung.0600"
                                | "melderegisterauskunfteinfach.antwort.0601"
                                | "administration.returntosender.0900"
                                | "administration.returntosender.0910"
                        ) {
                            return Err(Error::Malformed(
                                "XTA envelope contains no message content".to_string(),
                            ));
                        }
                        return Err(Error::UnknownMessage {
                            root,
                            namespace: ns,
                        });
                    }
                    // A self-closing root carries no fachlich content.
                    assert_known_namespace(&ns, &root)?;
                    match root.as_str() {
                        "melderegisterauskunfteinfach.anforderung.0600"
                        | "melderegisterauskunfteinfach.antwort.0601"
                        | "administration.returntosender.0900"
                        | "administration.returntosender.0910" => {
                            return Err(Error::Malformed(format!(
                                "root element <{root}> is empty — no message content"
                            )));
                        }
                        other => {
                            return Err(Error::UnknownMessage {
                                root: other.to_string(),
                                namespace: ns.clone(),
                            });
                        }
                    }
                }
                Event::Eof => {
                    if in_envelope {
                        return Err(Error::Malformed(
                            "XTA envelope contains no message content".to_string(),
                        ));
                    }
                    return Err(Error::Malformed("no root element found".to_string()));
                }
                _ => {}
            }
        }
    }
}

fn assert_known_namespace(ns: &str, root: &str) -> Result<(), Error> {
    if KNOWN_NAMESPACES.contains(&ns) {
        Ok(())
    } else {
        Err(Error::UnknownMessage {
            root: root.to_string(),
            namespace: ns.to_string(),
        })
    }
}

/// Positioned right after the message root's Start was consumed (its name
/// already checked against the known namespaces): dispatch to the variant
/// parser.
fn dispatch_root(
    reader: &mut NsReader<&[u8]>,
    root: String,
    namespace: String,
) -> Result<Message, Error> {
    match root.as_str() {
        "melderegisterauskunfteinfach.anforderung.0600" => {
            AnfrageEcho::from_reader(reader, root).map(Message::Anfrage0600)
        }
        "melderegisterauskunfteinfach.antwort.0601" => {
            Antwort::from_reader(reader, root).map(Message::Antwort0601)
        }
        "administration.returntosender.0900" | "administration.returntosender.0910" => {
            Rts::from_reader(reader, root).map(Message::ReturnToSender)
        }
        other => Err(Error::UnknownMessage {
            root: other.to_string(),
            namespace,
        }),
    }
}

/// A parsed 0600 Anfrage (echo of the create surface; round-trip checks
/// cover it — correlations and Suchprofil names).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AnfrageEcho {
    /// zeichennachricht — the caller's message-level correlation ID.
    pub zeichennachricht: Option<String>,
    /// anwenderkennung — logger identity of the requesting user (header
    /// level; the official reference messages carry it).
    pub anwenderkennung: Option<String>,
    /// One echo per auskunft.anforderung, in order.
    pub einzelfaelle: Vec<EinzelanfrageEcho>,
}

/// Echo of one Einzelanfrage inside a 0600.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EinzelanfrageEcho {
    /// zeicheneinzelfall of the Einzelanfrage.
    pub zeicheneinzelfall: Option<String>,
    /// Suchprofil nachname (AllgemeinerName payload).
    pub nachname: Option<String>,
    /// Suchprofil vornamen (AllgemeinerName payload).
    pub vornamen: Option<String>,
}

/// A parsed 0601 Antwort.
#[derive(Debug, Clone, PartialEq)]
pub struct Antwort {
    /// Raw antwortstatus code („00“ = bearbeitet; classification maps the rest).
    pub antwortstatus: String,
    /// Echoed message-level correlation ID.
    pub zeichennachricht: Option<String>,
    /// auftragsnummer for Wiederholanfragen, if the Behörde sends one.
    pub auftragsnummer: Option<String>,
    /// One entry per angefragte Person, in message order.
    pub positionen: Vec<AuskunftPosition>,
}

impl Antwort {
    /// Message-level classification. A non-„00“ antwortstatus classifies the
    /// whole message (fachlich rejection for 03/04, technical fault for
    /// everything else — unknown codes included, conservatively technical).
    /// With „00“, the standard has no message-level semantics beyond the
    /// per-position results; the rule here: exactly one position (the
    /// normal 0600 flow) → that position's verdict; several positions →
    /// the first position's verdict, deterministically. Sammelnachrichten
    /// should classify per position via [`Antwort::positionen`] and
    /// [`AuskunftPosition::classify`] instead.
    pub fn classify(&self) -> Classification {
        let status_verdict = status_classification(&self.antwortstatus);
        if let Some(v) = status_verdict {
            return v;
        }
        match self.positionen.first() {
            Some(p) => p.classify(),
            None => Classification::Neutral,
        }
    }

    /// Per-position classifications, in message order. Only meaningful for
    /// antwortstatus „00“ (otherwise the positions were not processed).
    pub fn classify_positions(&self) -> Vec<Classification> {
        self.positionen.iter().map(|p| p.classify()).collect()
    }
}

/// Map the message-level antwortstatus; `None` for „00“ (bearbeitet —
/// classify the positions).
fn status_classification(code: &str) -> Option<Classification> {
    match code {
        "00" => None,
        "03" | "04" => Some(Classification::Rejected(Rejection {
            quelle: RejectionSource::Antwortstatus,
            code: code.to_string(),
        })),
        _ => Some(Classification::TechnicalFault(TechnicalFault {
            quelle: FaultSource::Antwortstatus,
            code: code.to_string(),
        })),
    }
}

/// One `auskunft.antwort` position: the echoed Suchprofil plus its
/// ergebnis or rueckweisung.
#[derive(Debug, Clone, PartialEq)]
pub struct AuskunftPosition {
    /// zeicheneinzelfall of the echoed Anfragedaten (caller's correlation).
    pub zeicheneinzelfall: Option<String>,
    /// auftragspositionsnummer for Wiederholanfragen, if given.
    pub auftragspositionsnummer: Option<String>,
    /// steuerungsinformationen/anzahlderverbleibendenversuche — how many
    /// repeat requests the Behörde still allows for this position.
    pub anzahl_verbleibende_versuche: Option<u64>,
    /// The fachlich outcome of this position.
    pub ergebnis: PositionErgebnis,
}

impl AuskunftPosition {
    /// Classification of this position from its ergebnis/rueckweisung:
    /// ergebnisstatus „01“ → Fulfilled, other known-or-unknown ergebnisstatus
    /// values → Neutral, rueckweisung → Rejected.
    pub fn classify(&self) -> Classification {
        match &self.ergebnis {
            PositionErgebnis::Daten(e) => {
                if e.ergebnisstatus == "01" {
                    Classification::Fulfilled(e.clone())
                } else {
                    Classification::Neutral
                }
            }
            PositionErgebnis::Rueckweisung(code) => Classification::Rejected(Rejection {
                quelle: RejectionSource::NichterstellungEinzelanfrage,
                code: code.clone(),
            }),
        }
    }
}

/// The per-position outcome choice from the schema:
/// ergebnis (status + data) or rueckweisung (Nichterstellung code).
#[derive(Debug, Clone, PartialEq)]
pub enum PositionErgebnis {
    /// The `ergebnis` branch — status code, Zusatzinformationen, person
    /// data when identified.
    Daten(Ergebnis),
    /// The `rueckweisung` branch — raw Nichterstellung.grund code.
    Rueckweisung(String),
}

/// A parsed administration.returntosender message.
#[derive(Debug, Clone, PartialEq)]
pub struct Rts {
    /// Which RTS variant („administration.returntosender.0900“ or „0910“).
    pub variant: String,
    /// begruendung/grund — the RTS code (T*, X*, S*, V*…).
    pub grund: String,
    /// ergaenzende.hinweise free texts, in order.
    pub hinweise: Vec<String>,
    /// The bounced original message, base64 as sent (the Behörde echoes
    /// the unprocessable bytes back).
    pub nachricht_base64: Option<String>,
}

impl Rts {
    /// RTS messages are always technical faults; the code rides along.
    pub fn classify(&self) -> Classification {
        Classification::TechnicalFault(TechnicalFault {
            quelle: FaultSource::ReturnToSender,
            code: self.grund.clone(),
        })
    }
}

// ─── reader plumbing ────────────────────────────────────────────────────

/// Path-scoped reader: dispatches on child local names while tracking a
/// path stack for error messages.
struct PathReader<'a, 'b> {
    reader: &'a mut NsReader<&'b [u8]>,
    path: Vec<String>,
}

fn local_name(start: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(start.local_name().as_ref()).into_owned()
}

impl<'a, 'b> PathReader<'a, 'b> {
    /// Positioned right after the root's Start was consumed.
    fn new(reader: &'a mut NsReader<&'b [u8]>, root: String) -> Self {
        PathReader {
            reader,
            path: vec![root],
        }
    }

    fn where_am_i(&self) -> String {
        self.path.join("/")
    }

    fn error(&self, msg: &str) -> Error {
        Error::Malformed(format!("{}: {msg}", self.where_am_i()))
    }

    /// Consume the current element's content up to and including its End,
    /// dispatching each direct child to `on`. `on` receives the child's
    /// local name and whether it was self-closing; it must consume the
    /// child's content entirely (helpers like [`text_content`] and nested
    /// `children` calls do).
    fn children<F>(&mut self, mut on: F) -> Result<(), Error>
    where
        F: FnMut(&mut Self, &str, bool) -> Result<(), Error>,
    {
        loop {
            let event = self.reader.read_event().map_err(|e| {
                Error::Malformed(format!("xml error under {}: {e}", self.where_am_i()))
            })?;
            match event {
                Event::Start(start) => {
                    let name = local_name(&start);
                    self.path.push(name.clone());
                    on(self, &name, false)?;
                    self.path.pop();
                }
                Event::Empty(start) => {
                    let name = local_name(&start);
                    self.path.push(name.clone());
                    on(self, &name, true)?;
                    self.path.pop();
                }
                Event::End(_) => {
                    // The only End we can see here is our own element's —
                    // child Ends are consumed by the dispatch callbacks.
                    self.path.pop();
                    return Ok(());
                }
                Event::Eof => {
                    return Err(self.error("unexpected end of document"));
                }
                _ => {}
            }
        }
    }
}

/// Consume everything up to and including the current element's End
/// without looking at it (Start/Empty already consumed by the dispatch).
fn skip_element(pr: &mut PathReader<'_, '_>) -> Result<(), Error> {
    let depth = pr.path.len();
    loop {
        let event = pr
            .reader
            .read_event()
            .map_err(|e| Error::Malformed(format!("xml error while skipping: {e}")))?;
        match event {
            Event::Start(_) => pr.path.push(String::new()),
            Event::End(_) => {
                pr.path.pop();
                if pr.path.len() < depth {
                    return Ok(());
                }
            }
            Event::Eof => {
                return Err(Error::Malformed(
                    "unexpected EOF while skipping".to_string(),
                ));
            }
            _ => {}
        }
    }
}

/// Text of a dispatched child: empty when self-closing, otherwise read
/// through its End.
fn element_text(
    pr: &mut PathReader<'_, '_>,
    is_empty: bool,
    context: &str,
) -> Result<String, Error> {
    if is_empty {
        return Ok(String::new());
    }
    let mut out = String::new();
    loop {
        let event = pr
            .reader
            .read_event()
            .map_err(|e| Error::Malformed(format!("xml error in {context}: {e}")))?;
        match event {
            Event::Text(t) => {
                let decoded =
                    pr.reader.decoder().decode(&t).map_err(|e| {
                        Error::Malformed(format!("text decoding in {context}: {e}"))
                    })?;
                out.push_str(&decoded);
            }
            Event::CData(t) => {
                let decoded =
                    pr.reader.decoder().decode(&t).map_err(|e| {
                        Error::Malformed(format!("cdata decoding in {context}: {e}"))
                    })?;
                out.push_str(&decoded);
            }
            Event::End(_) => return Ok(out.trim().to_string()),
            Event::Start(_) | Event::Empty(_) => {
                return Err(Error::Malformed(format!(
                    "unexpected child element inside {context}"
                )));
            }
            Event::Eof => return Err(Error::Malformed(format!("unexpected EOF inside {context}"))),
            _ => {}
        }
    }
}

/// Read an xoev code element's value: the wrapper Start was consumed by
/// the dispatch; expect exactly one `code` child (qualified wrappers,
/// unqualified child) and return its text. Consumes the wrapper's End,
/// per the dispatch span contract.
fn code_value(pr: &mut PathReader<'_, '_>, is_empty: bool) -> Result<String, Error> {
    if is_empty {
        return Err(pr.error("code wrapper element must not be self-closing"));
    }
    let mut value: Option<String> = None;
    loop {
        let event = pr
            .reader
            .read_event()
            .map_err(|e| Error::Malformed(format!("xml error in code element: {e}")))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let name = local_name(&start);
                if name == "code" && value.is_none() {
                    value = Some(element_text(pr, empty, "code")?);
                } else {
                    return Err(pr.error(&format!("unexpected element <{name}> in code wrapper")));
                }
            }
            Event::End(_) => {
                return value.ok_or_else(|| pr.error("code element without <code> child"));
            }
            Event::Eof => {
                return Err(Error::Malformed(
                    "unexpected EOF in code element".to_string(),
                ));
            }
            _ => {}
        }
    }
}

/// Read an AllgemeinerName payload: `<name xmlns="">text</name>`,
/// `<nichtVorhanden xmlns="">true</nichtVorhanden>`, or self-closing.
/// Consumes the wrapper's End, per the dispatch span contract.
fn allgemeiner_name_text(pr: &mut PathReader<'_, '_>, is_empty: bool) -> Result<String, Error> {
    if is_empty {
        return Ok(String::new());
    }
    let mut value: Option<String> = None;
    loop {
        let event = pr
            .reader
            .read_event()
            .map_err(|e| Error::Malformed(format!("xml error in AllgemeinerName: {e}")))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let name = local_name(&start);
                match name.as_str() {
                    "name" if value.is_none() => {
                        value = Some(element_text(pr, empty, "AllgemeinerName")?);
                    }
                    "nichtVorhanden" if value.is_none() => {
                        element_text(pr, empty, "nichtVorhanden")?;
                        value = Some(String::new());
                    }
                    _ => {
                        return Err(pr.error(&format!(
                            "expected <name> or <nichtVorhanden>, found <{name}>"
                        )));
                    }
                }
            }
            Event::End(_) => {
                return value.ok_or_else(|| pr.error("AllgemeinerName without <name> child"));
            }
            Event::Eof => {
                return Err(Error::Malformed(
                    "unexpected EOF in AllgemeinerName".to_string(),
                ));
            }
            _ => {}
        }
    }
}

// ─── 0600 ───────────────────────────────────────────────────────────────

impl AnfrageEcho {
    fn from_reader(reader: &mut NsReader<&[u8]>, root: String) -> Result<AnfrageEcho, Error> {
        let mut msg = AnfrageEcho {
            zeichennachricht: None,
            anwenderkennung: None,
            einzelfaelle: Vec::new(),
        };
        let mut pr = PathReader::new(reader, root);
        pr.children(|pr, name, is_empty| match name {
            "nachrichtenkopf" => skip_element(pr),
            "zeichennachricht" => {
                msg.zeichennachricht = Some(element_text(pr, is_empty, "zeichennachricht")?);
                Ok(())
            }
            "anwenderkennung" => {
                msg.anwenderkennung = Some(element_text(pr, is_empty, "anwenderkennung")?);
                Ok(())
            }
            "antragsteller" => skip_element(pr),
            "auskunft.anforderung" => {
                msg.einzelfaelle.push(einzelanfrage_echo(pr, is_empty)?);
                Ok(())
            }
            other => Err(pr.error(&format!("unexpected element <{other}> in 0600"))),
        })?;
        Ok(msg)
    }
}

fn einzelanfrage_echo(
    pr: &mut PathReader<'_, '_>,
    is_empty: bool,
) -> Result<EinzelanfrageEcho, Error> {
    let mut echo = EinzelanfrageEcho {
        zeicheneinzelfall: None,
        nachname: None,
        vornamen: None,
    };
    if is_empty {
        return Ok(echo);
    }
    pr.children(|pr, name, is_empty| match name {
        "anfragedaten" | "steuerungsinformationen" => skip_element(pr),
        "datenZurAnfrage" => daten_zur_anfrage(pr, is_empty, &mut echo),
        "technische.einzelidentifikation" => {
            if is_empty {
                return Ok(());
            }
            pr.children(|pr, name, is_empty| match name {
                "ereigniszeitpunkt" => {
                    element_text(pr, is_empty, "ereigniszeitpunkt")?;
                    Ok(())
                }
                "zeicheneinzelfall" => {
                    echo.zeicheneinzelfall = Some(element_text(pr, is_empty, "zeicheneinzelfall")?);
                    Ok(())
                }
                other => Err(pr.error(&format!(
                    "unexpected element <{other}> in technische.einzelidentifikation"
                ))),
            })
        }
        other => Err(pr.error(&format!(
            "unexpected element <{other}> in auskunft.anforderung"
        ))),
    })?;
    Ok(echo)
}

fn daten_zur_anfrage(
    pr: &mut PathReader<'_, '_>,
    is_empty: bool,
    echo: &mut EinzelanfrageEcho,
) -> Result<(), Error> {
    if is_empty {
        return Ok(());
    }
    pr.children(|pr, name, is_empty| match name {
        "name" => {
            if !is_empty {
                pr.children(|pr, name, is_empty| match name {
                    "nachname" => {
                        echo.nachname = Some(allgemeiner_name_text(pr, is_empty)?);
                        Ok(())
                    }
                    "vornamen" => {
                        echo.vornamen = Some(allgemeiner_name_text(pr, is_empty)?);
                        Ok(())
                    }
                    "phonetik" => Ok(()),
                    other => {
                        Err(pr.error(&format!("unexpected element <{other}> in Suchprofil.Name")))
                    }
                })?;
            }
            Ok(())
        }
        "anschrift"
        | "geburtsdatum"
        | "ordensname"
        | "kuenstlername"
        | "geburtsortUndStaat"
        | "geschlechtOderFamilienstand"
        | "gesetzlicherVertreter.name"
        | "einzugsdatum"
        | "auszugsdatum"
        | "familienstandBeginnUndOrt"
        | "partner.name"
        | "sterbetag"
        | "sterbeortUndStaat" => skip_element(pr),
        other => Err(pr.error(&format!("unexpected element <{other}> in datenZurAnfrage"))),
    })
}

// ─── 0601 ───────────────────────────────────────────────────────────────

impl Antwort {
    fn from_reader(reader: &mut NsReader<&[u8]>, root: String) -> Result<Antwort, Error> {
        let mut msg = Antwort {
            antwortstatus: String::new(),
            zeichennachricht: None,
            auftragsnummer: None,
            positionen: Vec::new(),
        };
        let mut pr = PathReader::new(reader, root);
        pr.children(|pr, name, is_empty| match name {
            "nachrichtenkopf" => skip_element(pr),
            "antwortstatus" => {
                msg.antwortstatus = code_value(pr, is_empty)?;
                Ok(())
            }
            "zeichennachricht" => {
                msg.zeichennachricht = Some(element_text(pr, is_empty, "zeichennachricht")?);
                Ok(())
            }
            "auftragsnummer" => {
                msg.auftragsnummer = Some(element_text(pr, is_empty, "auftragsnummer")?);
                Ok(())
            }
            "auskunft.antwort" => {
                msg.positionen.push(auskunft_position(pr, is_empty)?);
                Ok(())
            }
            other => Err(pr.error(&format!("unexpected element <{other}> in 0601"))),
        })?;
        if msg.antwortstatus.is_empty() {
            return Err(Error::Malformed(
                "antwortstatus is mandatory in 0601 but missing".to_string(),
            ));
        }
        Ok(msg)
    }
}

fn auskunft_position(
    pr: &mut PathReader<'_, '_>,
    is_empty: bool,
) -> Result<AuskunftPosition, Error> {
    let mut pos = AuskunftPosition {
        zeicheneinzelfall: None,
        auftragspositionsnummer: None,
        anzahl_verbleibende_versuche: None,
        ergebnis: PositionErgebnis::Daten(neutral_ergebnis()),
    };
    if is_empty {
        return Ok(pos);
    }
    let mut have_ergebnis = false;
    pr.children(|pr, name, is_empty| match name {
        "auftragspositionsnummer" => {
            pos.auftragspositionsnummer =
                Some(element_text(pr, is_empty, "auftragspositionsnummer")?);
            Ok(())
        }
        "auskunft.angefordert" => {
            let echo = einzelanfrage_echo(pr, is_empty)?;
            pos.zeicheneinzelfall = echo.zeicheneinzelfall;
            Ok(())
        }
        "ergebnis" => {
            have_ergebnis = true;
            pos.ergebnis = ergebnis_choice(pr, is_empty)?;
            Ok(())
        }
        "steuerungsinformationen" => {
            if !is_empty {
                pr.children(|pr, inner, is_empty| match inner {
                    "anzahlderverbleibendenversuche" => {
                        let text = element_text(pr, is_empty, "anzahlderverbleibendenversuche")?;
                        pos.anzahl_verbleibende_versuche = text.parse::<u64>().ok();
                        Ok(())
                    }
                    _ => skip_element(pr),
                })?;
            }
            Ok(())
        }
        other => Err(pr.error(&format!("unexpected element <{other}> in auskunft.antwort"))),
    })?;
    if !have_ergebnis {
        // Schema: ergebnis is optional — e.g. when the request was not
        // processed. Classify as neutral without data.
        pos.ergebnis = PositionErgebnis::Daten(neutral_ergebnis());
    }
    Ok(pos)
}

fn neutral_ergebnis() -> Ergebnis {
    Ergebnis {
        ergebnisstatus: "05".to_string(),
        ..Ergebnis::default()
    }
}

/// The `type.Melderegisterauskunft.Ergebnis` choice: `ergebnis` (data) or
/// `rueckweisung` (Nichterstellung code).
fn ergebnis_choice(pr: &mut PathReader<'_, '_>, is_empty: bool) -> Result<PositionErgebnis, Error> {
    if is_empty {
        return Err(pr.error("ergebnis choice must not be self-closing"));
    }
    let mut daten: Option<Ergebnis> = None;
    let mut rueckweisung: Option<String> = None;
    pr.children(|pr, name, is_empty| match name {
        "ergebnis" => {
            let mut e = neutral_ergebnis();
            if !is_empty {
                pr.children(|pr, name, is_empty| match name {
                    "ergebnisstatus" => {
                        e.ergebnisstatus = code_value(pr, is_empty)?;
                        Ok(())
                    }
                    "zusatzinformation" => {
                        e.zusatzinformationen.push(code_value(pr, is_empty)?);
                        Ok(())
                    }
                    "namen" => {
                        e.namen = Some(person_name(pr, is_empty)?);
                        Ok(())
                    }
                    "anschrift.aktuell" => {
                        if let Some(a) = anschrift_melderecht(pr, is_empty)? {
                            e.anschriften.push(a);
                        }
                        Ok(())
                    }
                    other => Err(pr.error(&format!("unexpected element <{other}> in ergebnis"))),
                })?;
            }
            if e.ergebnisstatus.is_empty() {
                return Err(pr.error("ergebnisstatus is mandatory inside ergebnis"));
            }
            daten = Some(e);
            Ok(())
        }
        "rueckweisung" => {
            rueckweisung = Some(code_value(pr, is_empty)?);
            Ok(())
        }
        other => Err(pr.error(&format!(
            "unexpected element <{other}> in type.Melderegisterauskunft.Ergebnis"
        ))),
    })?;
    if let Some(code) = rueckweisung {
        return Ok(PositionErgebnis::Rueckweisung(code));
    }
    daten.map(PositionErgebnis::Daten).ok_or_else(|| {
        Error::Malformed("ergebnis choice: neither ergebnis nor rueckweisung found".to_string())
    })
}

/// type.Melderegisterauskunft.Name: familienname.unstrukturiert,
/// vornamen, gebraeuchlicherVorname, doktorgrad (+ tolerated extras).
fn person_name(pr: &mut PathReader<'_, '_>, is_empty: bool) -> Result<PersonName, Error> {
    let mut name = PersonName::default();
    if is_empty {
        return Ok(name);
    }
    pr.children(|pr, name_elem, is_empty| match name_elem {
        "familienname.unstrukturiert" => {
            if !is_empty {
                let mut nachname = None;
                pr.children(|pr, inner, is_empty| match inner {
                    "nachname" => {
                        nachname = Some(allgemeiner_name_text(pr, is_empty)?);
                        Ok(())
                    }
                    "nachnamepass" => skip_element(pr),
                    other => Err(pr.error(&format!(
                        "unexpected element <{other}> in FamiliennameUnstrukturiert"
                    ))),
                })?;
                name.familienname = nachname;
            }
            Ok(())
        }
        "vornamen" => {
            if !is_empty {
                let mut vornamen = None;
                pr.children(|pr, inner, is_empty| match inner {
                    "vornamen" => {
                        vornamen = Some(allgemeiner_name_text(pr, is_empty)?);
                        Ok(())
                    }
                    "vornamenPass" => skip_element(pr),
                    other => {
                        Err(pr.error(&format!("unexpected element <{other}> in type.Vornamen")))
                    }
                })?;
                name.vornamen = vornamen;
            }
            Ok(())
        }
        "gebraeuchlicherVorname" => {
            name.gebraeuchlicher_vorname = Some(allgemeiner_name_text(pr, is_empty)?);
            Ok(())
        }
        "doktorgrad" => {
            name.doktorgrad = Some(element_text(pr, is_empty, "doktorgrad")?);
            Ok(())
        }
        "ehename.unstrukturiert"
        | "lebenspartnerschaftsname.unstrukturiert"
        | "geburtsname.unstrukturiert"
        | "frueherer.familienname.unstrukturiert"
        | "ordensname"
        | "kuenstlername" => skip_element(pr),
        other => Err(pr.error(&format!(
            "unexpected element <{other}> in type.Melderegisterauskunft.Name"
        ))),
    })?;
    Ok(name)
}

/// type.AnschriftMelderecht: choice of anschrift.inland / anschrift.ausland
/// / anschrift.unbekannt. Only the Inland form is surfaced (the German
/// Melderegister flow); the other forms yield None.
fn anschrift_melderecht(
    pr: &mut PathReader<'_, '_>,
    is_empty: bool,
) -> Result<Option<Anschrift>, Error> {
    let mut result: Option<Anschrift> = None;
    if is_empty {
        return Ok(result);
    }
    pr.children(|pr, child, is_empty| match child {
        "anschrift.inland" => {
            let mut a = Anschrift::default();
            if !is_empty {
                pr.children(|pr, field, is_empty| match field {
                    "gemeindeschluessel" => {
                        a.ags = Some(code_value(pr, is_empty)?);
                        Ok(())
                    }
                    "hausnummer" => {
                        a.hausnummer = Some(element_text(pr, is_empty, "hausnummer")?);
                        Ok(())
                    }
                    "postleitzahl" => {
                        a.postleitzahl = Some(element_text(pr, is_empty, "postleitzahl")?);
                        Ok(())
                    }
                    "strasse" => {
                        a.strasse = Some(element_text(pr, is_empty, "strasse")?);
                        Ok(())
                    }
                    "wohnort" => {
                        a.wohnort = Some(element_text(pr, is_empty, "wohnort")?);
                        Ok(())
                    }
                    _ => skip_element(pr),
                })?;
            }
            result = Some(a);
            Ok(())
        }
        "anschrift.ausland" | "anschrift.unbekannt" => skip_element(pr),
        other => Err(pr.error(&format!(
            "unexpected element <{other}> in type.AnschriftMelderecht"
        ))),
    })?;
    Ok(result)
}

// ─── RTS ────────────────────────────────────────────────────────────────

impl Rts {
    fn from_reader(reader: &mut NsReader<&[u8]>, root: String) -> Result<Rts, Error> {
        let mut msg = Rts {
            variant: root.clone(),
            grund: String::new(),
            hinweise: Vec::new(),
            nachricht_base64: None,
        };
        let mut pr = PathReader::new(reader, root);
        pr.children(|pr, name, is_empty| match name {
            "nachrichtenkopf" => skip_element(pr),
            "rts.container" => {
                if is_empty {
                    return Err(pr.error("rts.container must not be self-closing"));
                }
                pr.children(|pr, child, is_empty| match child {
                    "rueckweisende.stelle"
                    | "transportinformationen"
                    | "nicht.verarbeitbarer.einzelfall"
                    | "ursprungsnachricht" => skip_element(pr),
                    "begruendung" => {
                        if is_empty {
                            return Err(pr.error("begruendung must not be self-closing"));
                        }
                        pr.children(|pr, inner, is_empty| match inner {
                            "grund" => {
                                msg.grund = code_value(pr, is_empty)?;
                                Ok(())
                            }
                            "ergaenzende.hinweise" => {
                                msg.hinweise.push(element_text(
                                    pr,
                                    is_empty,
                                    "ergaenzende.hinweise",
                                )?);
                                Ok(())
                            }
                            other => {
                                Err(pr
                                    .error(&format!("unexpected element <{other}> in begruendung")))
                            }
                        })
                    }
                    "nachricht" => {
                        msg.nachricht_base64 = Some(element_text(pr, is_empty, "nachricht")?);
                        Ok(())
                    }
                    other => {
                        Err(pr.error(&format!("unexpected element <{other}> in rts.container")))
                    }
                })
            }
            other => Err(pr.error(&format!("unexpected element <{other}> in RTS"))),
        })?;
        if msg.grund.is_empty() {
            return Err(Error::Malformed(
                "begruendung/grund is mandatory in an RTS message but missing".to_string(),
            ));
        }
        Ok(msg)
    }
}
