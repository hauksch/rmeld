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
            anschrift: Some(rmeld::AnschriftInput {
                ags: Some("010530088888".to_string()),
                hausnummer: Some("12".to_string()),
                postleitzahl: "24103".to_string(),
                strasse: "K.außenstraße".to_string(),
                wohnort: "Kiel".to_string(),
            }),
            ..Default::default()
        }],
        ereigniszeitpunkt: Some("2026-11-16T12:00:00+01:00".to_string()),
        attribute: Default::default(),
    }
}

/// The writer indents; assertions on wire shapes read a flattened copy.
/// Whitespace is dropped only between tags — spaces inside text content
/// („Maurice Alessio“) survive.
fn flat(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut prev_tag_closed = false;
    for c in xml.chars() {
        if c.is_whitespace() && prev_tag_closed {
            continue;
        }
        prev_tag_closed = c == '>';
        out.push(c);
    }
    out
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
    c.anfragen[0].anschrift.as_mut().unwrap().postleitzahl = String::new();
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

// ─── phase 2: the full Suchprofil ───────────────────────────────────────

#[test]
fn suchprofil_without_anschrift_passes_with_two_criteria() {
    // The Suchregel's second arm: name + two further criteria, no Anschrift.
    let mut c = base_input();
    c.anfragen[0].anschrift = None;
    c.anfragen[0].geburtsdatum = Some("1968".parse().expect("partial date"));
    c.anfragen[0].geschlecht_oder_familienstand = Some(
        rmeld::GeschlechtOderFamilienstand::Geschlecht("m".to_string()),
    );
    let xml = Anfrage::build(c)
        .expect("valid")
        .to_xml()
        .expect("serialize");
    rmeld::validate(xml.as_bytes())
        .expect("name + zwei weitere Kriterien ohne Anschrift must pass the official XSD");
    assert!(xml.contains("<geschlechtOderFamilienstand>"));
    assert!(xml.contains("<jahr xmlns=\"\">1968</jahr>"));
}

#[test]
fn suchregel_rejects_lone_criterion_without_anschrift() {
    let mut c = base_input();
    c.anfragen[0].anschrift = None;
    c.anfragen[0].geburtsdatum = Some("1968-03".parse().expect("partial date"));
    let err = Anfrage::build(c).expect_err("one criterion without Anschrift must fail");
    assert!(matches!(err, rmeld::Error::InvalidRequest(_)), "got {err}");
    assert!(err.to_string().contains("Suchregel"), "got {err}");
}

#[test]
fn familienstand_variant_of_the_choice_validates() {
    let mut c = base_input();
    c.anfragen[0].geschlecht_oder_familienstand = Some(
        rmeld::GeschlechtOderFamilienstand::Familienstand("VH".to_string()),
    );
    let xml = Anfrage::build(c)
        .expect("valid")
        .to_xml()
        .expect("serialize");
    rmeld::validate(xml.as_bytes()).expect("familienstand VH must pass the official XSD");
    let flat = flat(&xml);
    assert!(
        flat.contains("<familienstand><code xmlns=\"\">VH</code></familienstand>"),
        "fragment: {flat}"
    );
}

#[test]
fn maximal_suchprofil_validates() {
    // Every criterion at once — the strongest statement the XSD gate can
    // make about the create surface.
    let mut c = base_input();
    c.anfragen[0].ordensname = Some("Bruder Klaus".to_string());
    c.anfragen[0].kuenstlername = Some("Klaus Bühler".to_string());
    c.anfragen[0].geburtsort = Some(rmeld::OrtMitStaat {
        ort: "Rom".to_string(),
        staat_code: Some("ITA".to_string()),
    });
    c.anfragen[0].gesetzlicher_vertreter = Some(rmeld::GesetzlicherVertreter::Natuerlich {
        familienname: "Mustervertreter".to_string(),
        vornamen: "Vera".to_string(),
    });
    c.anfragen[0].einzugsdatum = Some("2001-03-01".parse().expect("date"));
    c.anfragen[0].auszugsdatum = Some("2003-07-31".parse().expect("date"));
    c.anfragen[0].familienstand_beginn = Some(rmeld::FamilienstandBeginn {
        datum: "1999-05".parse().expect("partial date"),
        ort: "Flensburg".to_string(),
        staat_code: None,
        datum_vorausgegangene_lp: Some("1995-01-01".parse().expect("date")),
    });
    c.anfragen[0].partner = Some(rmeld::Partner {
        familienname: Some("Partnername".to_string()),
        geburtsname: Some("Geburtsname".to_string()),
        vornamen: Some("Petra".to_string()),
        doktorgrad: Some("Dr.".to_string()),
    });
    c.anfragen[0].sterbetag = Some("2020-12-24".parse().expect("date"));
    c.anfragen[0].sterbeort = Some(rmeld::OrtMitStaat {
        ort: "Wien".to_string(),
        staat_code: Some("AUT".to_string()),
    });
    c.anfragen[0].option_auskunft = vec!["01".to_string()];
    c.anfragen[0].verzicht_auf_mitteilung = true;
    c.anfragen[0].gewerblicher_zweck = Some(rmeld::GewerblicherZweck {
        codes: vec!["4".to_string()],
        freitext: None,
        geschaeftszeichen: "GZ-2026-01".to_string(),
    });
    let xml = Anfrage::build(c)
        .expect("valid")
        .to_xml()
        .expect("serialize");
    rmeld::validate(xml.as_bytes())
        .expect("the maximal Suchprofil must pass the official XSD in full");
    // Spot-check the trickier wire shapes against the official forms
    // (flattened: the writer indents).
    let flat = flat(&xml);
    assert!(flat.contains("<geburtsortUndStaat><geburtsort>Rom</geburtsort>"));
    assert!(flat.contains("ITA"), "staat code must appear");
    assert!(flat.contains("<steuerungsinformationen>"));
    assert!(flat.contains("<verzichtAufMitteilung>true</verzichtAufMitteilung>"));
    assert!(flat.contains("<gewerblicherZweck>"));
    assert!(flat.contains("<sterbetag><sterbetag>2020-12-24</sterbetag></sterbetag>"));
    assert!(flat.contains("<code listVersionID=\"1\"><code xmlns=\"\">4</code></code>"));
}

#[test]
fn gewerblicher_zweck_fails_fast_on_incomplete_input() {
    let mut c = base_input();
    c.anfragen[0].gewerblicher_zweck = Some(rmeld::GewerblicherZweck {
        codes: vec![],
        freitext: None,
        geschaeftszeichen: "GZ".to_string(),
    });
    let err = Anfrage::build(c).expect_err("Zweck without codes/freitext must fail");
    assert!(matches!(err, rmeld::Error::InvalidRequest(_)), "got {err}");

    let mut c = base_input();
    c.anfragen[0].gewerblicher_zweck = Some(rmeld::GewerblicherZweck {
        codes: vec!["8".to_string()],
        freitext: None,
        geschaeftszeichen: " ".to_string(),
    });
    let err = Anfrage::build(c).expect_err("Zweck without geschaeftszeichen must fail");
    assert!(matches!(err, rmeld::Error::InvalidRequest(_)), "got {err}");
}

#[test]
fn familienstand_beginn_rejects_unbekannt_datum() {
    let mut c = base_input();
    c.anfragen[0].familienstand_beginn = Some(rmeld::FamilienstandBeginn {
        datum: "unbekannt".parse().expect("partial date"),
        ort: "X".to_string(),
        staat_code: None,
        datum_vorausgegangene_lp: None,
    });
    let err = Anfrage::build(c).expect_err("unbekannt is not a TeilbekanntesDatum arm");
    assert!(err.to_string().contains("unbekannt"), "got {err}");
}

#[test]
fn plain_dates_reject_everything_but_iso() {
    let err = "17.03.1968"
        .parse::<rmeld::Datum>()
        .expect_err("German format");
    assert!(err.contains("YYYY-MM-DD"), "got {err}");
    let ok: rmeld::Datum = "2020-12-24".parse().expect("ISO date");
    assert_eq!(ok.as_str(), "2020-12-24");
}

#[test]
fn json_contract_carries_the_full_suchprofil() {
    let json = r#"{
        "zeichennachricht": "V-9",
        "empfaenger_behoerdenkennung": "010530088888",
        "absender_bezeichnung": "Muster GbR",
        "anfragen": [{
            "nachname": "Süß",
            "vornamen": "Maurice Alessio",
            "geburtsdatum": "1976-10-25",
            "geburtsort": {"ort": "Rom", "staat_code": "ITA"},
            "geschlecht_oder_familienstand": "d",
            "gewerblicher_zweck": {"codes": ["1"], "geschaeftszeichen": "GZ-1"}
        }]
    }"#;
    let input: AnfrageInput =
        serde_json::from_str(json).expect("JSON contract with full Suchprofil");
    let xml = Anfrage::build(input)
        .expect("build")
        .to_xml()
        .expect("serialize");
    rmeld::validate(xml.as_bytes())
        .expect("JSON-driven maximal surface must pass the official XSD");
    let flat = flat(&xml);
    assert!(
        flat.contains("<name xmlns=\"\">Süß</name>"),
        "Umlaute byte-exact through the JSON path"
    );
    assert!(
        flat.contains("<geschlecht listVersionID=\"1\"><code xmlns=\"\">d</code></geschlecht>")
    );
}
