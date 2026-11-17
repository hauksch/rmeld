//! `rmeld` — the fachlich layer of German Melderegister communication,
//! scoped to the „Einfache Melderegisterauskunft“ message family.
//!
//! [docs/STANDARD-NOTES.md] documents the standard facts this crate encodes.
//! The short version: the hand-off brief's „602.1“ does not exist; the
//! request is Nachricht **0600** („Anfrage zur einfachen
//! Melderegisterauskunft“) and the answer is Nachricht **0601** („Antwort
//! zur einfachen Melderegisterauskunft“), per the vendored XMeld 26.11
//! schemas ([schema/PROVENANCE.md]).
//!
//! Division of labor with the transport stack (rosci): payloads cross rosci
//! as opaque bytes; rmeld owns exactly the xMeld XML — create 0600 from
//! structured input, parse 0601 (and the technical-fault messages) into
//! typed results, classify them, and XSD-validate both directions offline
//! via xmllint and the embedded vendored schemas.

pub mod classification;
pub mod create;
pub mod error;
pub mod parse;
pub mod validate;
pub mod wire;

pub use classification::{
    Anschrift, Classification, Ergebnis, FaultSource, PersonName, Rejection, RejectionSource,
    TechnicalFault,
};
pub use create::{Anfrage, AnfrageInput, AnschriftInput, EinzelanfrageInput, PartialDatum};
pub use error::Error;
pub use parse::{Antwort, AuskunftPosition, EinzelanfrageEcho, Message, PositionErgebnis, Rts};
pub use validate::{SchemaKind, validate, validate_as};
