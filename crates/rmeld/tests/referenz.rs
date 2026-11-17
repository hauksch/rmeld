//! Cross-check against the official KoSIT reference messages
//! (XMeld-Testsuite, referenznachrichten FachspezifischEMRA001–006).
//!
//! These are the standard's own examples, shipped in the testsuite package
//! (osci.de testsuite ZIP, „Bestandteil der Testsuite des Standards
//! XMeld“). They use the XMeld 27.05 namespace with prefixed elements —
//! so this file is the proof of the parser's release-namespace tolerance
//! and of the wire understanding against non-synthetic input. The 26.11
//! XSD gate does not apply here (different release namespace; the 26.11
//! schemas would reject these roots by design).

use rmeld::{Classification, Message};

fn referenz(name: &str) -> Vec<u8> {
    let path = format!(
        "{env}/tests/fixtures/referenz/{name}",
        env = env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read reference message {path}: {e}"))
}

#[test]
fn official_0601_positives_are_fulfilled() {
    for name in [
        "FachspezifischEMRA001b-0601.xml",
        "FachspezifischEMRA002b-0601.xml",
        "FachspezifischEMRA005b-0601.xml",
        "FachspezifischEMRA006b-0601.xml",
    ] {
        let Message::Antwort0601(antwort) =
            Message::parse(&referenz(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
        else {
            panic!("{name}: expected 0601");
        };
        assert_eq!(antwort.antwortstatus, "00", "{name}");
        let verdict = antwort.classify();
        assert!(
            matches!(verdict, Classification::Fulfilled(_)),
            "{name}: expected Fulfilled, got {verdict:?}"
        );
    }
}

#[test]
fn official_0601_neutrals_are_neutral() {
    for name in [
        "FachspezifischEMRA003b-0601.xml",
        "FachspezifischEMRA004b-0601.xml",
    ] {
        let Message::Antwort0601(antwort) =
            Message::parse(&referenz(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
        else {
            panic!("{name}: expected 0601");
        };
        let verdict = antwort.classify();
        assert_eq!(verdict, Classification::Neutral, "{name}");
        assert!(
            verdict.text().contains("nicht oder derzeit nicht"),
            "{name}"
        );
    }
}

#[test]
fn official_0600_anfragen_round_trip_correlations() {
    for name in [
        "FachspezifischEMRA001a-0600.xml",
        "FachspezifischEMRA002a-0600.xml",
        "FachspezifischEMRA003a-0600.xml",
        "FachspezifischEMRA004a-0600.xml",
        "FachspezifischEMRA005a-0600.xml",
        "FachspezifischEMRA006a-0600.xml",
    ] {
        let Message::Anfrage0600(anfrage) =
            Message::parse(&referenz(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
        else {
            panic!("{name}: expected 0600");
        };
        assert!(
            !anfrage.einzelfaelle.is_empty(),
            "{name}: every 0600 carries at least one Einzelanfrage"
        );
        for einzel in &anfrage.einzelfaelle {
            assert!(
                einzel.nachname.is_some() || einzel.vornamen.is_some(),
                "{name}: Suchprofil names must surface"
            );
        }
    }
}

#[test]
fn out_of_family_reference_message_is_unknown_not_broken() {
    // EMRA006c is a 1322 Datenabruf — same test case, different message
    // family. rmeld is 0600/0601-scoped and must say so, loudly.
    let err = Message::parse(&referenz("FachspezifischEMRA006c-1322.xml"))
        .expect_err("1322 must not parse as a 0600/0601 message");
    assert!(
        matches!(err, rmeld::Error::UnknownMessage { .. }),
        "got {err}"
    );
}
