# Fixtures

## `*.xml`, `referenz/` — sources and status

Everything in this directory is free of real Personenstand data. Two
kinds:

1. **Synthetic** (`anfrage-0600.xml`, `antwort-0601-*.xml`, `rts-0900.xml`):
   hand-built from the vendored XMeld 26.11 schemas (schema/PROVENANCE.md),
   validated offline with xmllint against the official 26.11 XSDs, with
   obviously invented persons („Müller-Lüdenscheidt“, AGS 010530088888).
   They exist because the official testsuite has no reference messages
   for the rueckweisung, antwortstatus-fault and RTS variants.

2. **Official reference messages** (`referenz/FachspezifischEMRA001–006*`):
   verbatim from the KoSIT XMeld testsuite package
   ([osci.de testsuite](https://osci.de/xmeld2705/testsuite/), „Bestandteil
   der Testsuite des Standards XMeld“; fictional test persons by
   construction). They speak the XMeld **27.05** namespace with prefixed
   elements and are the cross-release proof for the parser; the 26.11 XSD
   gate deliberately does not apply to them.

The names in the synthetic fixtures deliberately carry ÄÖÜ/ß so the
create→parse round trip proves Umlaut byte-exactness. The neutral fixture
learned us the hard way that DIN 91379 `datatypeC` rejects the en-dash
(–) — the character subset is enforced by the schema gate, not by this
crate.

| File | Message | Variant |
|---|---|---|
| anfrage-0600.xml | 0600 Anfrage | baseline Suchprofil (name + Anschrift + Geburtsdatum) |
| antwort-0601-positiv.xml | 0601 Antwort | ergebnisstatus 01, Zusatzinformation 01, name + Anschrift |
| antwort-0601-neutral.xml | 0601 Antwort | ergebnisstatus 05 — keine Personenidentifikation |
| antwort-0601-rueckweisung.xml | 0601 Antwort | rueckweisung, Nichterstellung.grund 08 |
| antwort-0601-status-nichtberechtigt.xml | 0601 Antwort | antwortstatus 03 (keine Berechtigung), ohne Positionen |
| antwort-0601-status-technisch.xml | 0601 Antwort | antwortstatus 01 (Anfragefehler), ohne Positionen |
| rts-0900.xml | administration.returntosender.0900 | RTS, Grund T099 |
| referenz/FachspezifischEMRA00{1,2,5,6}b | 0601 Antwort (27.05) | official, ergebnisstatus 01 |
| referenz/FachspezifischEMRA00{3,4}b | 0601 Antwort (27.05) | official, ergebnisstatus 05 |
| referenz/FachspezifischEMRA00{1..6}a | 0600 Anfrage (27.05) | official, Sammel- und Einzelanfragen |
| referenz/FachspezifischEMRA006c-1322.xml | 1322 Datenabruf (27.05) | official, out-of-family → UnknownMessage |
