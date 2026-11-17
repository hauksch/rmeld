//! Create side: JSON input → 0600 XML → the official XSD gate → parse
//! round trip. The Umlaut byte-exactness requirement lives here.

// Tests panic on broken assumptions — that is the job description.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use rmeld::{Anfrage, AnfrageInput, EinzelanfrageInput, Message, PartialDatum};

fn base_input() -> AnfrageInput {
    AnfrageInput {
        zeichennachricht: "V-2026-4711".to_string(),
        empfaenger_behoerdenkennung: "010530088888".to_string(),
        absender_bezeichnung: "Musterermittlungen GbR".to_string(),
        anfragen: vec![EinzelanfrageInput {
            zeicheneinzelfall: Some("V-2026-4711-1".to_string()),
            nachname: "Müller-Lüdenscheidt".to_string(),
            vornamen: "Jürgen Örlich".to_string(),
            geburtsdatum: Some("1968-03-17".parse().expect("partial date")),
            anschrift: rmeld::AnschriftInput {
                ags: Some("010530088888".to_string()),
                hausnummer: Some("12".to_string()),
                postleitzahl: "24103".to_string(),
                strasse: "K.außenstraße".to_string(),
                wohnort: "Kiel".to_string(),
            },
        }],
        ereigniszeitpunkt: Some("2026-11-16T12:00:00+01:00".to_string()),
        attribute: Default::default(),
    }
}

#[test]
fn created_anfrage_passes_the_official_xsd() {
    let anfrage = Anfrage::build(base_input()).expect("valid input");
    let xml = anfrage.to_xml().expect("serialize");
    rmeld::validate(xml.as_bytes()).expect("rmeld-built 0600 must pass the official XSD");
}

#[test]
fn created_anfrage_round_trips_umlauts_byte_exact() {
    let anfrage = Anfrage::build(base_input()).expect("valid input");
    let xml = anfrage.to_xml().expect("serialize");

    let Message::Anfrage0600(parsed) = Message::parse(xml.as_bytes()).expect("re-parse") else {
        panic!("expected 0600 echo");
    };
    assert_eq!(parsed.zeichennachricht.as_deref(), Some("V-2026-4711"));
    assert_eq!(parsed.einzelfaelle.len(), 1);
    let echo = &parsed.einzelfaelle[0];
    // Byte-exact: the Strings are compared as UTF-8 bytes, covering ÄÖÜ/ß
    // in nachname, vornamen, strasse and the absender designation.
    assert_eq!(echo.nachname.as_deref(), Some("Müller-Lüdenscheidt"));
    assert_eq!(echo.vornamen.as_deref(), Some("Jürgen Örlich"));
    let xml_bytes = xml.as_bytes();
    assert!(
        xml_bytes
            .windows("Müller-Lüdenscheidt".len())
            .any(|w| w == "Müller-Lüdenscheidt".as_bytes()),
        "Umlauts must survive serialization byte-exact"
    );
    // The absender designation only lives in the header (not parsed back),
    // so check it on the wire bytes directly.
    assert!(
        xml.contains("<name>Musterermittlungen GbR</name>"),
        "absender designation must be present qualified"
    );
}

#[test]
fn partial_birthdates_map_to_the_right_wire_elements() {
    let input = base_input();
    let cases = [
        ("1968-03-17", "jahrMonatTag"),
        ("1968-03", "jahrMonat"),
        ("1968", "jahr"),
    ];
    for (value, element) in cases {
        let mut c = input.clone();
        c.anfragen[0].geburtsdatum = Some(value.parse().expect("partial date"));
        let xml = Anfrage::build(c)
            .expect("build")
            .to_xml()
            .expect("serialize");
        assert!(
            xml.contains(&format!("<{element} xmlns=\"\">{value}</{element}>")),
            "{value} must serialize as <{element}>"
        );
        rmeld::validate(xml.as_bytes()).expect("partial date must still validate");
    }

    let mut c = input.clone();
    c.anfragen[0].geburtsdatum = Some("unbekannt".parse().expect("unbekannt"));
    let xml = Anfrage::build(c)
        .expect("build")
        .to_xml()
        .expect("serialize");
    assert!(xml.contains("<unbekannt xmlns=\"\">true</unbekannt>"));
    rmeld::validate(xml.as_bytes()).expect("unbekannt must still validate");
}

#[test]
fn invalid_input_fails_fast_before_any_xml() {
    // No Einzelanfragen.
    let mut c = base_input();
    c.anfragen.clear();
    let err = Anfrage::build(c).expect_err("empty Anfragen must fail");
    assert!(matches!(err, rmeld::Error::InvalidRequest(_)), "got {err}");

    // Anschrift fields empty.
    let mut c = base_input();
    c.anfragen[0].anschrift.postleitzahl = String::new();
    let err = Anfrage::build(c).expect_err("empty PLZ must fail");
    assert!(matches!(err, rmeld::Error::InvalidRequest(_)), "got {err}");

    // Empty Behördenkennung.
    let mut c = base_input();
    c.empfaenger_behoerdenkennung = "  ".to_string();
    let err = Anfrage::build(c).expect_err("blank Behoerdenkennung must fail");
    assert!(matches!(err, rmeld::Error::InvalidRequest(_)), "got {err}");

    // Bogus partial date.
    let err = "17.03.1968"
        .parse::<PartialDatum>()
        .expect_err("German-format date must fail");
    assert!(err.contains("geburtsdatum"), "got {err}");
}

#[test]
fn json_input_deserializes_into_the_contract() {
    let json = r#"{
        "zeichennachricht": "V-1",
        "empfaenger_behoerdenkennung": "010530088888",
        "absender_bezeichnung": "Muster GbR",
        "anfragen": [{
            "nachname": "Müller",
            "vornamen": "Jürgen",
            "anschrift": {"postleitzahl": "24103", "strasse": "K.außenstraße", "wohnort": "Kiel"}
        }]
    }"#;
    let input: AnfrageInput = serde_json::from_str(json).expect("JSON contract");
    let anfrage = Anfrage::build(input).expect("build");
    let xml = anfrage.to_xml().expect("serialize");
    rmeld::validate(xml.as_bytes()).expect("minimal JSON input must produce a valid 0600");
    assert!(
        xml.contains("fassung=\"2026-01-31\""),
        "default fassung applies"
    );
    assert!(xml.contains("version=\"26.11\""), "default version applies");
}
