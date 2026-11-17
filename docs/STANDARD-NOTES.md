# Standard notes — what rmeld encodes about xMeld, and how it was verified

Answers two questions: which standard facts does this repo implement, and
which of them were verified against official artifacts versus assumed.
Every claim below cites the vendored schema files or the official
package it came from; the hand-off brief's working assumptions are
corrected where they were wrong.

## 0. The 602.1 correction — read this first

The hand-off brief's premise — „message type **602.1** =
Einfache Melderegisterauskunft“ — is **wrong**, and no amount of caution
makes 602.1 exist: zero hits in the specification texts (XMeld 1.7
through 27.05), all XSD sets, and the official XMeld-Nachrichten code
list (`urn:de:xmeld:schluesseltabelle:xmeld.nachrichten`). The correct
message pair for the simple register request, per the global element
declarations in
`schema/xmeld/xmeld-nachrichten-melderegisterauskunft.xsd`:

| Nachrichtennummer | Element | Official name (spec IV.10.6) |
|---|---|---|
| **0600** | `melderegisterauskunfteinfach.anforderung.0600` | „Anfrage zur einfachen Melderegisterauskunft“ |
| **0601** | `melderegisterauskunfteinfach.antwort.0601` | „Antwort zur einfachen Melderegisterauskunft“ |
| 0602–0604 | `…gemeindeuebergreifend.*` | gemeindeübergreifend family — **out of scope** for phase 1 |

The spec (OSCI-XMeld 3.4, p. 1063f) is unambiguous: „Die Antwort auf die
Nachricht 0600 ist jeweils nur die Nachricht 0601“. One plausible origin
of the wrong number: 0602 is the gemeindeübergreifend Anfrage — but the
brief names „Einfache Melderegisterauskunft“, and that is 0600/0601,
full stop. The README repeats this correction.

## 1. Version decision — XMeld 26.11

- Releases switched from 3.x to date-based (25.05, 26.11, …); each has a
  fixed „gültig ab“ date. **26.11 is binding since 2026-11-01** (KoSIT
  version overview, www1.osci.de/meldewesen/xmeld-2827); 27.05 is
  published (Fassung 2026-07-31) but binding only from 2027-05-01.
- rmeld vendors, validates and creates in **26.11**
  (`http://www.osci.de/xmeld2611`). The parser accepts the known release
  namespaces (34, 2605, 2611, 2705) — the answer arriving in a
  neighbouring release still parses; anything else fails loudly.
- **Assumption to verify with the human/intermediary:** which release the
  actual Meldebehörde/intermediary operates. The „gültig ab“ schedule
  governs implementations, not each authority's deployed version. The
  parser's namespace tolerance is the designed defense.

## 2. Schema set — composition and provenance

Vendored under `schema/` (see `schema/PROVENANCE.md` for sources,
retrieval date and checksums):

- xMeld 26.11: master `xinneres.xmeld.xsd` + `xmeld-basistypen`,
  `xmeld-baukasten`, `xmeld-codes` + 16 `xmeld-nachrichten-*.xsd`.
- The Auskunft family imports **no XJustiz and no xdienst** — the
  hand-off brief assumed otherwise. Its actual import closure: XInneres
  modules (allgemeinername/4, auslandsanschrift/5, basisnachricht/7,
  behörde/7, gemeindeverzeichnis-codes/3, datum/2, geschlecht/1, idnr/2,
  meldeanschrift/5, nachrichtentypen/7, postanschrift/5, rueckweisung/6)
  plus XÖV DIN SPEC 91379 datatypes (2022-08), Basisnachricht
  unqualified g2g 1.1 and xoev-code 1.0.
- The schemas compile **as shipped** under libxml2 2.15 — the
  deterministic-content-model trap from the OSCI schemas next door does
  not bite here. Recorded as an explicit negative finding in
  PROVENANCE.md; only line-ending normalization deviates from upstream.
- Official reference messages exist: the KoSIT testsuite ships six
  0600/0601 pairs (FachspezifischEMRA001–006), vendored under
  `crates/rmeld/tests/fixtures/referenz/` and parsed/classified by tests
  — the parser is proven against non-synthetic input, in the 27.05
  namespace with prefixed elements.

## 3. Wire facts the code depends on

All verified by iterating instances against the official XSDs (the
fixtures validate; every fixture in tests/fixtures proves its rule):

- **Namespace zones**: xmeld structural elements are qualified
  (`elementFormDefault="qualified"`); leaf data from the XInneres modules
  and the xoev-code `<code>` children are **unqualified** (serialized
  with `xmlns=""`); the g2g-Basisnachricht leaves (kanal, kennung) are
  unqualified too while their xmeld wrappers stay qualified. This mixed
  form is the main interoperability trap of the whole standard.
- **Root attributes**: `fassung` (spec production date, YYYY-MM-DD),
  `version` („26.11“), `produkt`, `produkthersteller` are required;
  `id`, `produktversion`, `test` optional
  (`xmeld:type.basisnachricht`).
- **0600 structure** (`type.datenuebermittlung.privat2b`):
  `nachrichtenkopf` = identifikation.nachricht (with Nachrichtennummer
  code 0600) + absender (Kunde) + empfaenger (Behörde, Behördenkennung
  required); then zeichennachricht?, anwenderkennung?, antragsteller?,
  1..n `auskunft.anforderung` with anfragedaten (incl. the mandatory
  `zumZweckDerWerbungOderAdresshandel=false` declaration),
  datenZurAnfrage (Suchprofil) and technische.einzelidentifikation —
  whose `zeicheneinzelfall` is **mandatory on the wire** even though a
  Fachanwendung thinks of it as optional (rmeld fills
  `{zeichennachricht}-{position}` when absent).
- **0601 structure** (`type.datenuebermittlung.b2privat`): mandatory
  `antwortstatus`; 0..n `auskunft.antwort` positions, each echoing the
  Anforderungsdaten and carrying the choice `ergebnis`
  (ergebnisstatus + zusatzinformationen + namen? + anschrift.aktuell*)
  or `rueckweisung`.
- **Code lists** (urn:de:xmeld:schluesseltabelle:*, values live in the
  schema enumerations, official texts quoted in
  `crates/rmeld/src/classification.rs`):
  - `antwortstatus` v2 (message level): 00 erbracht · 01 Anfragefehler ·
    03 keine Berechtigung · 04 Gebührenregelung · 05 nicht unterstützt.
  - `melderegisterauskunft.ergebnisstatus` v3 (per Anfrage): 01
    eindeutig identifiziert · 05 Auskunft nicht erteilt (neutral; deckt
    Auskunftssperren § 51/§ 52 BMG). The „keine Personenidentifikation“
    phrasing of older releases is gone; codes 02/03/04 were removed.
  - `melderegisterauskunft.nichterstellung.grund` v5 (rueckweisung):
    01, 04–10 (08 ungültiges Suchprofil, 10 fehlende
    Werbung/Adresshandel-Erklärung, …).
  - `melderegisterauskunft.zusatzinformation` v2: 01–06 (andere Adresse,
    andere Namen, verstorben, manuelle Bearbeitung …).
  - RTS `ruecksendung.einer.nachricht` v5: S* nicht spezifikationskonform,
    T* Zertifikat/Transport, X* XML/Schlüsseltabellen, V* Version.
- **DIN 91379 `datatypeC` enforces its character subset**: the en-dash
  (–) is rejected by pattern validation. Caught by the XSD gate on a
  fixture; input text with typographic dashes must be transliterated
  upstream.
- **Namespace drift between releases**: 26.11 uses
  `http://www.osci.de/xmeld2611`; the testsuite's 27.05 messages use
  `http://www.osci.de/xmeld2705` with prefixed (not default-namespace)
  elements. Both parse; creation emits 26.11 only.

## 4. Classification taxonomy (rmeld's contract)

```
Classification ::= Fulfilled(Ergebnis)     — ergebnisstatus 01, person data
                  | Neutral                — ergebnisstatus 05 / fehlendes Ergebnis
                  | Rejected(Rejection)    — rueckweisung code or antwortstatus 03/04
                  | TechnicalFault(...)    — antwortstatus 01/05/* or RTS 0900/0910
```

`antwortstatus != 00` classifies the whole message; with 00 the
per-position results rule (see `Antwort::classify` docs for the
Sammelnachrichten rule). Unknown code values keep their raw string —
the lists are versioned and may grow; rmeld never invents a verdict for
a code it has not seen.

## 5. Codegen decision (recorded, not folklore)

`xsd-parser` 1.8 was spiked against this schema set (the hand-off brief
asked for a look before hand-rolling). Result: generated code does not
compile — duplicate struct fields (`list_uri`, `list_version_id`,
`produkt`, …) on every xoev-code restriction type, because XSD allows
redeclaring base attributes in a restriction and the generator renders
both. Plus it needed URL-rewriting/custom resolvers for the absolute
import locations. Decision: **hand-rolled quick-xml serialization**, with
the XSD validation gate as the acceptance test for wire fidelity. The
spike artifacts were discarded; this section is the record.

## 6. Validation path

xmllint (libxml2) + OASIS catalog over the vendored schemas, embedded in
the binary (`include_str!`) and materialized into a process-lifetime
temp dir — offline by construction, no path assumptions. xmllint lookup:
`RMELD_XMLLINT` → `PATH` → `~/.local/bin/xmllint`; missing tool is a
loud error, never a silent skip. libxml2 ≥ 2.13 does not fetch remote
URLs at all, so the catalog is mandatory.

## 7. XTA envelope — deferred, deliberately

XTA („XÖV Transportauftrag“, XRepository `urn:xoev-de:kosit:standard:xta2`)
is the wrapping the intermediary sees; rosci treats the payload as opaque
bytes and merely sniffs the `<XTA>` root. Phase 1: `rmeld create` emits
the **bare 0600 xMeld message**; the XTA-2 envelope and the „XTA …“-style
Betreff are a contract with the intermediary, not with rosci. When the
intermediary's convention is on the table, the wrap belongs inside
rmeld's create output — phase-2 candidate, schema not vendored yet.

## 8. What rests on assumptions (honest list)

1. **Which release the intermediary actually runs** — unverifiable
   online; the release schedule says 26.11 is binding since 2026-11-01.
   Namespace tolerance + `fassung`/`version` attributes are the defenses.
2. **`fassung` value semantics**: set to the release's Fassung date
   („2026-01-31“ for 26.11, per the KoSIT version table). The schema only
   demands a datatypeC string; real-world values should match the spec's
   production date — worth one look at a live message from the
   intermediary before go-live.
3. **Kommunikation.kanal code values** (e.g. „001“ for phone in the
   fixtures) come from an open XÖV code list whose genericode file was
   not vendored; the schema enforces no enumeration. Verify against the
   intermediary's DVDV/Kommunikation conventions.
4. **GemeindeVZ `listVersionID`** is pinned to „20260701“ in create —
   a constant, not configuration, until a real DVDV/GemeindeVZ vintage
   exists to align with.
5. **Phase-1 Suchprofil scope**: name + Anschrift + Geburtsdatum (full or
   partial). The schema's rarer criteria (Ordensname, Partnername,
   Sterbetag, Einzugsdatum, …) are not exposed yet; absence is
   documented, not silent.
6. **CI has not run yet** — no git remote is configured, so the
   workflows (fmt/clippy/test/cargo-deny on GitHub runners with libxml2
   2.15) are exercised locally only. Push to GitHub to arm them.

## 9. Source register

- KoSIT XMeld version hub: https://www1.osci.de/meldewesen/xmeld-2827
- XMeld 26.11 release page: https://www1.osci.de/meldewesen/xmeld/xmeld-26-11-27836
- XMeld 26.11 XSD mirror (vendored): https://osci.de/xmeld2611/
- XMeld testsuite (reference messages): https://osci.de/xmeld2705/testsuite/
- XRepository XMeld 26.11: https://www.xrepository.de/details/urn:xoev-de:kosit:standard:xinneres.xmeld_26.11
- XInneres modules (vendored): https://www.osci.de/xinneres/
- DIN 91379 datatypes (vendored): https://xoev.de/schemata/din/91379/2022-08/din-norm-91379-datatypes.xsd
- OSCI-XMeld 3.4 specification (message inventory, PDF): https://www.xrepository.de/details/urn:xoev-de:kosit:standard:xinneres.xmeld_3.4
- XTA 2 standard (deferred envelope): https://www.xrepository.de/details/urn:xoev-de:kosit:standard:xta2_3
