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
  1..n `auskunft.anforderung` with anfragedaten (gewerblicherZweck?,
  zeicheneinzelfall?, the mandatory
  `zumZweckDerWerbungOderAdresshandel=false` declaration),
  datenZurAnfrage (the full Suchprofil — see below) and
  technische.einzelidentifikation — whose `zeicheneinzelfall` is
  **mandatory on the wire** even though a Fachanwendung thinks of it as
  optional (rmeld fills `{zeichennachricht}-{position}` when absent).
  `steuerungsinformationen` sits **after** datenZurAnfrage in the
  sequence (option.auskunft codes 01/04, wiederholanfrage reference to a
  previous order — `auftragsnummer.alteanfrage`/
  `auftragspositionsnummer.alteanfrage`, verzichtAufMitteilung; the
  answer-side steering (`auftragsnummer` message level,
  `auftragspositionsnummer` + `anzahlderverbleibendenversuche` per
  position) is what a Wiederholanfrage references).
- **Suchprofil** (datenZurAnfrage, in schema sequence order): name
  (nachname + vornamen, both required on the wire), anschrift? (Inland:
  AGS?, hausnummer?, PLZ/Strasse/Wohnort required), ordensname?,
  kuenstlername?, geburtsdatum? (xida:TeilbekanntesDatumMitUnbekannt —
  `teilbekanntesDatum` wrapper with jahrMonatTag/jahrMonat/jahr, or
  `unbekannt=true`), geburtsortUndStaat? (geburtsort +
  geburtsortStaat as Code.Gebiet), geschlechtOderFamilienstand? (choice:
  Code.Geschlecht — `listVersionID` **required** — or Code.Familienstand),
  gesetzlicherVertreter.name?, einzugsdatum?/auszugsdatum? (plain
  xs:date), familienstandBeginnUndOrt? (datumBeginn as
  TeilbekanntesDatum — **no unbekannt arm** — plus Ort/Staat),
  partner.name?, sterbetag? (type.Sterbetag wrapper + inner
  sterbetag date), sterbeortUndStaat?. The element forms of the
  criteria were cross-checked against the official KoSIT reference
  messages (EMRA001–006: e.g. `<xmeld:einzugsdatum>2019-11-01` —
  qualified with text, no inner wrapper).
- **Suchregel** (datenZurAnfrage documentation): „Die Namensangaben sind
  verpflichtend, von den sonstigen Suchkriterien sind mindestens eine
  Anschrift, oder zwei der weiteren zulässigen Daten anzugeben.“ Which
  criteria count as „zulässig“ is Schematron territory; rmeld enforces
  the conservative superset (any two optional criteria qualify) and the
  Behörde keeps the final word (grund 08).
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
  - `melderegisterauskunft.gewerblicher.zweck` v1 (open list in the
    schema; values from the genericode file on XRepository): 1
    Adressabgleich, 2 Adressermittlung, 3 Adressabgleich für Dritte,
    4 Aktualisierung eigener Bestandsdaten, 5 Adresshistorisierung,
    6 Forderungsmanagement, 7 Bonitätsrisikoprüfungen, 8 Werbung,
    9 Adresshandel, 10 Markt-, Meinungs- oder Sozialforschung.
  - XInneres `geschlecht` v1 (open list in the schema; values from the
    genericode file): `m`, `w`, `x` (keine Angabe), `d` (divers).
  - DSMeld `familienstand` v2 (enumerated in the schema): `EA`, `GS`,
    `LA`, `LD`, `LE`, `LP`, `LV`, `NB`, `VH`, `VW`.
  - Destatis `staatsgebiete` and `ags` (open lists, schema pins no
    enumeration; `listVersionID` required on their code wrappers).
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

## 7. XTA envelope — researched verdict: stays outside rmeld

Phase 2 answered the hand-off's open question properly. What the XTA 2
standard (XRepository `urn:xoev-de:kosit:standard:xta2`, newest release
**Version 5.1.1**, AKTUELL) normatively defines:

- an XTA **Webservice** (xtacore.wsdl: sendMessage/getMessage/fetch…)
  between Fachverfahren and Transportdienstleister — not a file format;
- the `XTA-ServiceProfile` schemas (namespace
  `https://xoev.de/transport/xta/serviceprofile/1.3.1`) and the
  `MessageMetaData` object (namespace `http://www.osci.eu/ws/2014/10/
  transport`, `OSCI_MessageMetaData_V2.02.xsd`) as the representation of
  the Transportauftrag.

The `<XTA>`-root **file** that rosci sniffs is a de-facto container
convention of the transport infrastructure (the „XTA-Datei“ dropped for
pickup), **published by nobody as a schema**. rosci's own docs say the
same: envelope version + Betreff are „a contract with the intermediary,
not with rosci“, and rosci sends anything while merely warning.
rmeld implementing a guessed envelope would be exactly the
maybe-correct output the hand-off forbids.

So the recorded decision (superseding the phase-1 „inside rmeld is the
natural place“ hunch): **rmeld does not wrap**. The Transportauftrag
metadata is transport-side — rosci's `/v1/send` already carries it
(`metadata_author`/`metadata_reader`, the MessageMetaData identifiers).
What rmeld needs from the intermediary, and cannot derive from any
standard, is exactly two strings: the Betreff convention and, if it
differs from `<XTA>`, the envelope root — both go on the
intermediary-contract list in §8. If a contract document ever lands,
`create` growing an `--xta` wrapper is a contained phase-3 change.

**Phase-3 web research (2026-11-19), closing the question:** every
normative document in the family was checked for the two contract
strings, and none defines them.

- **Betreff**: normatively the optional OSCI 1.2 `Subject` header
  element (inside `storeDelivery`/`mediateDelivery`, xsd:string,
  „Betreff einer Zustellung“, OSCI-1.2-Spezifikation §storeDelivery;
  also `order.xsd` in the vendored OSCI schemas). Its *content* is a
  free string — the spec's own examples say „Betreff der Zustellung“,
  the 2026 MessageMetaData-Ergänzung ships „Subject0“. Checked and
  empty: OSCI-1.2 spec, XTA 2 V5.1.1 spec, XMeld 3.4 spec (its only
  `betreff` element is RTS message content), XMeld 26.11 Verbindliche
  Handlungsanweisung (fachlich only). The „XTA 2.4 Anzeige“-style
  examples circulating in practice are per-intermediary handbook
  conventions.
- **Envelope root**: the XTA lineage is Webservice-only (XTA-WS 1.1.1 →
  XTA 2), and the official KoSIT „XML-Beispiele für MessageMetaData“
  root at `<MessageMetaData>` — a Transportauftrag metadata object that
  per the 2026 Ergänzung hangs into the OSCI SOAP header via a
  `##other` wildcard in namespace
  `http://www.osci.eu/ws/2014/10/transport` (rosci's vendored schemas
  already reserve exactly that namespace for it). No normative `<XTA>`
  file container exists anywhere in the standards; it is an
  implementation convention of XTA/OSCI server file-drop adapters.
  Practical consequence for the contract talk: the routing information
  intermediaries actually act on lives in MessageMetaData fields
  (`Qualifier/Service/BusinessScenario`, `MessageType`,
  `Reader/Directory`) — DVDV-driven, not Betreff-driven — which rosci
  already transports.

So the two strings remain genuinely intermediary-specific, and the
search space is exhausted: the standards cannot provide them.

**Decision implemented (phase 3, after the research):** rmeld follows
the de-facto convention as far as it is observable — `rmeld create`
emits the transport file with the `<XTA>` envelope root (the element
rosci and the file-drop adapters expect) around the fachlich message,
and carries the Betreff as an attribute of that root:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<XTA betreff="">
  <melderegisterauskunfteinfach.anforderung.0600 ...>
```

`--betreff` takes the free OSCI Subject string and **defaults to
empty** — a contract-pending slot, never a fabricated value. The inner
layout is deliberately minimal (one root, one message child, one
attribute) so nothing can conflict with the intermediary's real
handbook; when that handbook arrives, only `to_xta` and this section
change. The rest of the pipeline is envelope-transparent: `validate`
and `parse` see through `<XTA>` (case-insensitive, any namespace, like
rosci's sniff) to the fachlich message and gate it against the official
XSDs; the library's `to_xml()` still emits the bare message.

## 8. What rests on assumptions (honest list)

1. **Which release the intermediary actually runs** — unverifiable
   online; the release schedule says 26.11 is binding since 2026-11-01.
   Namespace tolerance + `fassung`/`version` attributes are the defenses.
2. **`fassung` value semantics**: set to the release's Fassung date
   („2026-01-31“ for 26.11, per the KoSIT version table). The schema only
   demands a datatypeC string; real-world values should match the spec's
   production date — worth one look at a live message from the
   intermediary before go-live.
3. **Code-list vintages in create output** are constants, not
   configuration: GemeindeVZ „20260701“, geschlecht „1“ (verified
   against the genericode file), staatsgebiete „2“,
   gewerblicher.zweck „1“. A real DVDV/GemeindeVZ vintage to align with
   does not exist yet; when it does, these move to input fields.
4. **Suchprofil „zulässige Daten“ enumeration**: the Suchregel's
   authoritative list of what counts as a valid second/third criterion is
   Schematron-level (the XSD cannot express it). rmeld accepts a
   conservative superset; a Behörde-side rejection arrives as grund 08.
5. **Intermediary contract items** (cannot be derived from any standard —
   see §7 for the exhausted search): the **Betreff** content convention
   (normatively the free OSCI 1.2 `Subject` header element) and, if it
   differs from `<XTA>`, the **envelope root element** of the handed-over
   file. Needed verbatim from the intermediary before first live send.
   Related but standard-adjacent: which MessageMetaData
   BusinessScenario/MessageType values the intermediary expects on the
   routing side — rosci-side configuration, not rmeld's.
   Also affects `empfaenger_behoerdenkennung`: DVDV access is restricted
   to participants of the öffentlichen Verwaltung (Behörden, ≥ 50 %
   public-owned companies etc., per the DVDV Nutzungsvoraussetzungen) —
   a private actor requesting in its own name is not an eligible DVDV
   participant. The realistic topology for gewerbliche
   Melderegisterauskünfte is a broker (Vermittlungsdienst — e.g. ZEMA
   online/AKDB, pmZEMA/govconnect, or an XMeld intermediary) that holds
   the DVDV/OSCI connectivity itself, contracts with the Massenanfrager,
   and routes on the AGS in the Suchprofil. In that topology the
   Behördenkennung comes from the broker's onboarding documentation
   (constant gateway value or per-Behörde mapping), not from a DVDV
   lookup — one more item for the same contract conversation.
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
- XTA 2 standard (§7 verdict): https://www.xrepository.de/details/urn:xoev-de:kosit:standard:xta2 — current release Version 5.1.1 (`urn:…:xta2_5.1.1`, spec PDF 26.07.2024, bundle incl. WSDL + schemas + catalog)
- KoSIT „XML-Beispiele für MessageMetaData“ (root = `<MessageMetaData>`): https://www.xoev.de/osci-xta/standard-xta-2/xta-hilfsmittel/xml-beispiele-fuer-messagemetadata-23198
- OSCI 1.2 Spezifikation (Subject = „Betreff einer Zustellung“, free string): https://www.xoev.de/osci-xta/standard-osci-transport-1-2/osci-versionsuebersicht-23217
- Ergänzung „Übertragung von XTA2 MessageMetaData“ (2026, MMD in the OSCI SOAP header `##other`): https://www.xoev.de/sixcms/media.php/13/Erg%C3%A4nzung_zur_OSCI-MessageMetaData.pdf
- XMeld 26.11 Verbindliche Handlungsanweisung (fachlich only, no transport conventions): https://www1.osci.de/sixcms/media.php/13/2026-11-11_Handlungsanweisung_26.11-final.pdf
- XBasisdaten Anlage 2 (XTA-WS method mapping for register-like queries: sendMessageSync / request-response): https://www.xrepository.de/api/xrepository/urn:xoev-de:bva:standard:xbasisdaten:dokumentation:Anlage_2_Transportspezifische_Anforderungen_an_eine_sichere_Daten_bermittlung_mit_XBasisdaten
- XTA 2 gewerblicher-Zweck/geschlecht code lists: XRepository genericode endpoints, e.g. `https://www.xrepository.de/api/xrepository/urn:de:xmeld:schluesseltabelle:melderegisterauskunft.gewerblicher.zweck_1/dokumente` and `urn:xoev-de:xinneres:codeliste:geschlecht`
