# Fixtures — all synthetic

Every file in this directory is **synthetic**: hand-built from the
vendored XMeld 26.11 schemas (schema/PROVENANCE.md), validated offline
with xmllint against the official XSDs, and populated with obviously
invented persons („Müller-Lüdenscheidt“, AGS 010530088888). No real
Personenstand data, ever — the hand-off brief's zero-tolerance rule.

The names deliberately carry ÄÖÜ/ß so the create→parse round trip proves
Umlaut byte-exactness, and `antwort-0601-neutral.xml` learned us the
hard way that DIN 91379 `datatypeC` rejects the en-dash (–) — the
character subset is enforced by the schema gate, not by this crate.

| File | Message | Variant |
|---|---|---|
| anfrage-0600.xml | 0600 Anfrage | baseline Suchprofil (name + Anschrift + Geburtsdatum) |
| antwort-0601-positiv.xml | 0601 Antwort | ergebnisstatus 01, Zusatzinformation 01, name + Anschrift |
| antwort-0601-neutral.xml | 0601 Antwort | ergebnisstatus 05 — keine Personenidentifikation |
| antwort-0601-rueckweisung.xml | 0601 Antwort | rueckweisung, Nichterstellung.grund 08 |
| antwort-0601-status-nichtberechtigt.xml | 0601 Antwort | antwortstatus 03 (keine Berechtigung), ohne Positionen |
| antwort-0601-status-technisch.xml | 0601 Antwort | antwortstatus 01 (Anfragefehler), ohne Positionen |
| rts-0900.xml | administration.returntosender.0900 | RTS, Grund T099 |
