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
//! via xmllint and the vendored schemas.

pub mod error;

pub use error::Error;
