# Changelog

## Unreleased — phase 2

- Full Suchprofil in `create`: Geschlecht (m/w/x/d) oder Familienstand,
  Geburts-/Sterbeort und -staat, Sterbetag, Einzugs-/Auszugsdatum,
  Ordnens-/Künstlername, gesetzlicher Vertreter, Partnername,
  steuerungsinformationen (option.auskunft, verzichtAufMitteilung) und
  gewerblicher Zweck (Codes 1–10 / Freitext + Geschäftszeichen).
- The Suchregel („mindestens eine Anschrift, oder zwei der weiteren
  zulässigen Daten“) is enforced fail-fast; Anschrift is no longer
  mandatory when two further criteria are supplied.
- `parse`: `anzahlderverbleibendenversuche` (Wiederholanfrage steering)
  surfaces per Auskunft position.
- XTA verdict researched and recorded (docs/STANDARD-NOTES.md §7): the
  `<XTA>`-root container has no normative schema — the envelope stays an
  intermediary contract item; rmeld does not wrap.

## Unreleased — phase 1

- Vendored XMeld 26.11 schema set (+ XInneres modules, XÖV schemata)
  with offline xmllint catalog; provenance and checksums in
  `schema/PROVENANCE.md` / `schema/SHA256SUMS`.
- `crates/rmeld`: create 0600 „Anfrage zur einfachen
  Melderegisterauskunft" from JSON, parse 0601 „Antwort" and RTS
  0900/0910, classify into
  Fulfilled/Neutral/Rejected/TechnicalFault, XSD-validate offline via
  embedded schemas + xmllint.
- `crates/rmeld-cli` (`rmeld`): `create`, `check`, `validate` with
  fachlich exit codes (0/2/3/4/5/6) and `--json`.
- **Correction of the project brief**: „602.1" does not exist; the
  message pair is **0600/0601**. See `docs/STANDARD-NOTES.md`.
- Verified against the official XMeld schemas (fixtures + created
  messages) and the official KoSIT testsuite reference messages
  (FachspezifischEMRA001–006).
