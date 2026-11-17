//! The XSD gate: every fixture must validate against the official XMeld
//! 26.11 schemas, offline. A fixture that fails here means either the
//! vendored schema set or the wire understanding drifted — both are
//! release-blocking.

// Tests panic on broken assumptions — that is the job description.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use rmeld::validate;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{env}/tests/fixtures/{name}",
        env = env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"))
}

#[test]
fn anfrage_0600_validates() {
    validate(&fixture("anfrage-0600.xml")).expect("0600 fixture must pass the official XSD");
}

#[test]
fn antwort_0601_positiv_validates() {
    validate(&fixture("antwort-0601-positiv.xml"))
        .expect("positive 0601 fixture must pass the official XSD");
}

#[test]
fn antwort_0601_neutral_validates() {
    validate(&fixture("antwort-0601-neutral.xml"))
        .expect("neutral 0601 fixture must pass the official XSD");
}

#[test]
fn antwort_0601_rueckweisung_validates() {
    validate(&fixture("antwort-0601-rueckweisung.xml"))
        .expect("rueckweisung 0601 fixture must pass the official XSD");
}

#[test]
fn antwort_0601_status_nichtberechtigt_validates() {
    validate(&fixture("antwort-0601-status-nichtberechtigt.xml"))
        .expect("antwortstatus-03 fixture must pass the official XSD");
}

#[test]
fn antwort_0601_status_technisch_validates() {
    validate(&fixture("antwort-0601-status-technisch.xml"))
        .expect("antwortstatus-01 fixture must pass the official XSD");
}

#[test]
fn rts_0900_validates() {
    validate(&fixture("rts-0900.xml")).expect("RTS fixture must pass the official XSD");
}

#[test]
fn missing_xmllint_is_loud_not_silent() {
    // When no xmllint exists, validate() must return an error — never
    // Ok(()). This is the anti-"silently skipped gate" guard.
    match validate(&fixture("anfrage-0600.xml")) {
        Ok(()) => {} // xmllint present and fixture valid — the expected path
        Err(rmeld::Error::XmllintUnavailable { .. }) => {
            panic!("xmllint missing: install libxml2-utils (CI does; see README)")
        }
        Err(other) => panic!("unexpected validation error: {other}"),
    }
}
