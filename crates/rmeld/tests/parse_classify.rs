//! Parse + classify coverage: one test per response variant the standard
//! defines for the einfache Melderegisterauskunft flow, against fixtures
//! that all pass the official XSD (see validation.rs).

// Tests panic on broken assumptions — that is the job description.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use rmeld::{Classification, FaultSource, Message, PositionErgebnis, RejectionSource};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{env}/tests/fixtures/{name}",
        env = env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"))
}

fn parse(name: &str) -> Message {
    Message::parse(&fixture(name)).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

#[test]
fn positive_answer_is_fulfilled_with_person_data() {
    let Message::Antwort0601(antwort) = parse("antwort-0601-positiv.xml") else {
        panic!("expected 0601");
    };
    assert_eq!(antwort.antwortstatus, "00");
    assert_eq!(antwort.zeichennachricht.as_deref(), Some("V-2026-4711"));
    assert_eq!(antwort.positionen.len(), 1);

    let pos = &antwort.positionen[0];
    assert_eq!(pos.zeicheneinzelfall.as_deref(), Some("V-2026-4711-1"));
    let Classification::Fulfilled(ergebnis) = pos.classify() else {
        panic!("expected Fulfilled, got {:?}", pos.classify());
    };
    assert_eq!(ergebnis.ergebnisstatus, "01");
    assert_eq!(ergebnis.zusatzinformationen, vec!["01"]);
    let namen = ergebnis.namen.expect("fulfilled answer carries the name");
    assert_eq!(namen.familienname.as_deref(), Some("Müller-Lüdenscheidt"));
    assert_eq!(namen.vornamen.as_deref(), Some("Jürgen Örlich"));
    assert_eq!(ergebnis.anschriften.len(), 1);
    assert_eq!(ergebnis.anschriften[0].ags.as_deref(), Some("010530088888"));
    assert_eq!(
        ergebnis.anschriften[0].postleitzahl.as_deref(),
        Some("24103")
    );
    assert_eq!(
        ergebnis.anschriften[0].strasse.as_deref(),
        Some("K.außenstraße")
    );

    // Message-level classification delegates to the single position.
    assert_eq!(antwort.classify(), pos.classify());
    assert!(
        antwort
            .classify()
            .text()
            .starts_with("Der Betroffene wurde")
    );
}

#[test]
fn neutral_answer_classifies_neutral_without_person_data() {
    let Message::Antwort0601(antwort) = parse("antwort-0601-neutral.xml") else {
        panic!("expected 0601");
    };
    assert_eq!(antwort.positionen.len(), 1);
    let PositionErgebnis::Daten(e) = &antwort.positionen[0].ergebnis else {
        panic!("neutral answers ride in the ergebnis branch");
    };
    assert_eq!(e.ergebnisstatus, "05");
    assert!(e.namen.is_none(), "neutral answers carry no name");
    assert!(e.anschriften.is_empty());
    assert_eq!(antwort.classify(), Classification::Neutral);
}

#[test]
fn rueckweisung_classifies_rejected_with_grund() {
    let Message::Antwort0601(antwort) = parse("antwort-0601-rueckweisung.xml") else {
        panic!("expected 0601");
    };
    assert_eq!(
        antwort.classify(),
        Classification::Rejected(rmeld::Rejection {
            quelle: RejectionSource::NichterstellungEinzelanfrage,
            code: "08".to_string(),
        })
    );
    assert_eq!(
        antwort.classify().text(),
        "Das Suchprofil dieser Einzelanfrage ist ungültig oder wird nicht unterstützt."
    );
    // Wiederholanfrage steering rides on the position.
    assert_eq!(antwort.positionen[0].anzahl_verbleibende_versuche, Some(2));
}

#[test]
fn antwortstatus_03_is_a_message_level_rejection() {
    let Message::Antwort0601(antwort) = parse("antwort-0601-status-nichtberechtigt.xml") else {
        panic!("expected 0601");
    };
    assert_eq!(antwort.antwortstatus, "03");
    assert!(antwort.positionen.is_empty());
    let verdict = antwort.classify();
    assert_eq!(
        verdict,
        Classification::Rejected(rmeld::Rejection {
            quelle: RejectionSource::Antwortstatus,
            code: "03".to_string(),
        })
    );
    assert_eq!(verdict.text(), "Berechtigung für diese Anfrage fehlt.");
}

#[test]
fn antwortstatus_01_is_a_message_level_technical_fault() {
    let Message::Antwort0601(antwort) = parse("antwort-0601-status-technisch.xml") else {
        panic!("expected 0601");
    };
    assert_eq!(antwort.antwortstatus, "01");
    let verdict = antwort.classify();
    assert_eq!(
        verdict,
        Classification::TechnicalFault(rmeld::TechnicalFault {
            quelle: FaultSource::Antwortstatus,
            code: "01".to_string(),
        })
    );
    assert!(verdict.text().contains("Anfragefehler"));
}

#[test]
fn rts_classifies_technical_fault_with_rts_code() {
    let Message::ReturnToSender(rts) = parse("rts-0900.xml") else {
        panic!("expected RTS");
    };
    assert_eq!(rts.variant, "administration.returntosender.0900");
    assert_eq!(rts.grund, "T099");
    assert!(rts.hinweise[0].contains("keine echte Nachricht"));
    assert!(rts.nachricht_base64.is_some());
    let verdict = rts.classify();
    assert_eq!(
        verdict,
        Classification::TechnicalFault(rmeld::TechnicalFault {
            quelle: FaultSource::ReturnToSender,
            code: "T099".to_string(),
        })
    );
    assert!(
        verdict
            .text()
            .contains("Zertifikats- oder Transportproblem")
    );
}

#[test]
fn anfrage_round_trips_umlauts_byte_exact() {
    let Message::Anfrage0600(anfrage) = parse("anfrage-0600.xml") else {
        panic!("expected 0600");
    };
    assert_eq!(anfrage.zeichennachricht.as_deref(), Some("V-2026-4711"));
    assert_eq!(anfrage.einzelfaelle.len(), 1);
    assert_eq!(
        anfrage.einzelfaelle[0].nachname.as_deref(),
        Some("Müller-Lüdenscheidt")
    );
    assert_eq!(
        anfrage.einzelfaelle[0].vornamen.as_deref(),
        Some("Jürgen Örlich")
    );
    assert_eq!(
        anfrage.einzelfaelle[0].zeicheneinzelfall.as_deref(),
        Some("V-2026-4711-1")
    );
}

#[test]
fn unknown_message_is_rejected_loudly() {
    let bytes =
        b"<?xml version=\"1.0\"?><meldung.anderes.9999 xmlns=\"http://www.osci.de/xmeld2611\"/>";
    let err = Message::parse(bytes).expect_err("unknown roots must fail");
    assert!(
        matches!(err, rmeld::Error::UnknownMessage { .. }),
        "got {err}"
    );

    // Same root in a namespace we have never heard of — also rejected.
    let bytes =
        b"<?xml version=\"1.0\"?><meldung.anderes.9999 xmlns=\"http://example.org/fremd\"/>";
    let err = Message::parse(bytes).expect_err("unknown namespaces must fail");
    assert!(
        matches!(err, rmeld::Error::UnknownMessage { .. }),
        "got {err}"
    );
}

#[test]
fn broken_structure_is_malformed_not_panic() {
    // antwortstatus missing its <code> child.
    let bytes = b"<?xml version=\"1.0\"?><melderegisterauskunfteinfach.antwort.0601 xmlns=\"http://www.osci.de/xmeld2611\" fassung=\"2026-01-31\" version=\"26.11\" produkt=\"p\" produkthersteller=\"h\"><nachrichtenkopf><absender><behoerdenkennung>1</behoerdenkennung></absender><empfaenger><behoerdenkennung>2</behoerdenkennung></empfaenger></nachrichtenkopf><antwortstatus><falsch>00</falsch></antwortstatus></melderegisterauskunfteinfach.antwort.0601>";
    let err = Message::parse(bytes).expect_err("broken code element must fail");
    assert!(matches!(err, rmeld::Error::Malformed(_)), "got {err}");

    // Truncated document.
    let err = Message::parse(&fixture("antwort-0601-positiv.xml")[..200])
        .expect_err("truncated XML must fail");
    assert!(matches!(err, rmeld::Error::Malformed(_)), "got {err}");
}

// Silence unused-import lint for the type-only imports above.
#[allow(unused_imports)]
use rmeld::{Antwort as _, AuskunftPosition as _, PositionErgebnis as _, Rts as _};

#[test]
fn wrapped_answer_parses_and_classifies_through_the_envelope() {
    let inner = fixture("antwort-0601-positiv.xml");
    let wrapped = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<XTA betreff=\"\">\n{}\n</XTA>\n",
        String::from_utf8(inner.clone()).expect("fixture is utf-8")
    );
    // The XSD gate sees through the envelope.
    rmeld::validate(wrapped.as_bytes())
        .expect("wrapped answer must validate (envelope transparent)");
    let Message::Antwort0601(antwort) = Message::parse(wrapped.as_bytes()).expect("parse") else {
        panic!("expected 0601");
    };
    let Classification::Fulfilled(e) = antwort.classify() else {
        panic!("expected Fulfilled through the envelope");
    };
    assert_eq!(e.ergebnisstatus, "01");
    assert_eq!(e.zusatzinformationen, vec!["01"]);
    assert_eq!(
        e.namen.and_then(|n| n.familienname),
        Some("Müller-Lüdenscheidt".to_string())
    );
}

#[test]
fn empty_and_unknown_envelopes_fail_loudly() {
    let empty = b"<?xml version=\"1.0\"?><XTA betreff=\"\"></XTA>";
    let err = Message::parse(empty).expect_err("empty envelope must fail");
    assert!(err.to_string().contains("no message content"), "got {err}");

    let unknown_inner = b"<?xml version=\"1.0\"?><XTA><fremd.nachricht xmlns=\"http://www.osci.de/xmeld2611\"/></XTA>";
    let err = Message::parse(unknown_inner).expect_err("unknown inner root must fail");
    assert!(
        matches!(err, rmeld::Error::UnknownMessage { .. }),
        "got {err}"
    );
}
