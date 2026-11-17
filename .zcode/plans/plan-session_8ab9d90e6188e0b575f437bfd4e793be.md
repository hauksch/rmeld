# rmeld — implementation plan (phase 1)

## 0. Verified premise — the 602.1 correction

The handoff's working assumption is wrong and the handoff itself anticipated it: **„602.1" does not exist** in any xMeld release (1.7 → 27.05; zero hits in specs, XSDs, and the official Nachrichten code list). The correct message pair for „Einfache Melderegisterauskunft":

- **0600** `melderegisterauskunfteinfach.anforderung.0600` — „Anfrage zur einfachen Melderegisterauskunft"
- **0601** `melderegisterauskunfteinfach.antwort.0601` — „Antwort zur einfachen Melderegisterauskunft" (all answer variants ride in this one message)
- (0602–0604 = gemeindeübergreifend family — **out of scope** for phase 1)

Independently confirmed by fetching the official `xmeld-nachrichten-melderegisterauskunft.xsd`; will be re-verified against the downloaded spec PDF + XSDs during implementation. The correction goes prominently into README and `docs/STANDARD-NOTES.md`.

**Response taxonomy for 0601** (from spec IV.10 + code lists V.B.3.33–3.36, 3.43):
1. **Fulfilled** — `ergebnisstatus` = 01 (person uniquely identified, data transmitted), plus Zusatzinformation codes.
2. **Neutral / no identification** — `ergebnisstatus` = 05 („kann aus tatsächlichen oder rechtlichen Gründen nicht … erteilt werden"; covers Auskunftssperren §51/§52 BMG).
3. **Rejected (fachliche Nichterstellung)** — Nichterstellung-Grund codes 01–10 (06 missing entitlement, 08 invalid Suchprofil, 10 missing no-advertising declaration via element `rueckweisung`, …).
4. **Technical fault** — arrives as *different* messages: `administration.returntosender.0900/0910` (RTS reasons T/X/V/S), `datenuebermittlung.fehlerhafteNachricht.0508`; recognized and classified by element name.

**Schema facts**: targetNamespace is release-versioned (`http://www.osci.de/xmeld34`, `…xmeld2605`, `…xmeld2611`, `…xmeld2705`). The auskunft schema imports **no XJustiz/xdienst** (handoff assumption wrong) — it imports XInneres modules (name/4, gemeindeverzeichnis/3, datum/2, geschlecht/1, meldeanschrift/5) + DIN SPEC 91379 datatypes. An official testsuite exists (`osci.de/xmeld2705/testsuite/`) — check it for example-message fixtures before synthesizing any.

## 1. Decisions (defaults chosen; flag at approval if wrong)

- **Vendored/created release: xMeld 26.11** (binding since 01.11.2026). Parser accepts all known release namespaces (3.4, 26.05, 26.11, 27.05) so old answers still parse; creator emits 26.11.
- **XTA wrapping: deferred.** `rmeld create` outputs the bare 0600 xMeld message (UTF-8 XML). The XTA-2 „Transportauftrag" envelope + Betreff convention is an intermediary contract — decision + open items recorded in STANDARD-NOTES.md. (XTA 2 is itself a published standard; wrapping inside rmeld is the phase-2 candidate once the contract is known.)
- **Types: codegen spike first.** Try `xsd-parser` (Bergmann89, generates quick-xml code from XSD) on the vendored auskunft schema subset, timeboxed. If it handles the XÖV constructs cleanly → generated wire types behind a curated facade; otherwise hand-rolled quick-xml types for the 0600/0601 subset. Public API is curated either way.
- **Validation: xmllint + OASIS catalog** (rosci's proven pattern), offline via vendored schemas; schemas patched for libxml2 ≥ 2.13 deterministic-content-model rejection if needed, every deviation in `schema/PROVENANCE.md`. Missing xmllint = loud failure (pitfall 3 — never silently skip).
- **House style**: workspace like rosci — edition 2024, resolver 3, `rust-toolchain.toml` pin **1.98.1** + rustfmt/clippy, thiserror 2, clap 4 derive+env, tracing, no anyhow, `missing_docs = "deny"` on the lib, conventional commits, MIT.

## 2. Repo layout

```
Cargo.toml                  # workspace: crates/rmeld, crates/rmeld-cli
rust-toolchain.toml         # 1.98.1 + rustfmt, clippy
deny.toml                   # rosci shape (licenses/advisories/sources)
schema/                     # vendored xMeld 26.11 XSD set (+ imported modules), flat
schema/extern/              # XInneres module XSDs, DIN 91379, W3C externals
schema/catalog.xml          # OASIS catalog → offline resolution
schema/PROVENANCE.md        # rosci format: upstream, tag/version, retrieval date, deviations
crates/rmeld/               # lib: create / parse / classify / validate
crates/rmeld/tests/fixtures/  # 0600 + every 0601 variant + RTS 0900
crates/rmeld-cli/           # bin `rmeld`
docs/STANDARD-NOTES.md
scripts/vendor-schemas.sh   # reproducible download (manual, never in tests)
.github/workflows/{ci,audit}.yml, .github/dependabot.yml
README.md, LICENSE, .editorconfig, .gitattributes
```

## 3. Implementation phases

**Phase A — verify & vendor (no app code).** Download xMeld 26.11 package (XRepository `/dokumente` ZIP or osci.de mirror) + imported XInneres/DIN 91379 schemas; re-verify 0600/0601 and the taxonomy from the actual spec PDF; vendor + catalog + PROVENANCE.md; get xmllint to validate a sample 0600 offline (expect the libxml2 ≥ 2.13 determinism trap here — patch + document if it bites).

**Phase B — scaffold.** Workspace Cargo.toml, toolchain pin, deny.toml, CI copying rosci's `ci.yml`/`audit.yml` exactly (SHA-pinned actions **with explicit `toolchain: stable`**, rust-cache, `apt-get install libxml2-utils`, fmt/clippy `-D warnings`/test/cargo-deny; weekly audit + SBOM), dependabot, editorconfig/gitattributes.

**Phase C — library `crates/rmeld`.**
- `create`: `Request`/`Suchprofil` builder (name, vorname, geburtsdatum, anschrift, AGS, gewerblicher Zweck, Erklärung Werbung/Adresshandel…) → valid 0600 XML; fails fast on invariants.
- `parse`: 0601 (and RTS 0900/0910, 0508) → typed result (persons with Anschrift, ergebnisstatus, Zusatzinformationen).
- `classify`: `Classification` enum — `Fulfilled(..) | Neutral{grundangabe} | Rejected{grund} | TechnicalFault{kind}` mapped from the code lists.
- `validate`: xmllint + catalog wrapper, structured error output, loud on missing binary.
- Tests: one fixture per variant (official examples if usable, clearly-marked synthetic otherwise; **no real Personenstand data**), byte-exact Umlaute (ÄÖÜ/ß) create→parse round trip, namespace-tolerance tests across release namespaces, negative-parse tests ("negative tests are features").

**Phase D — CLI `rmeld`.** `rmeld create anfrage.json > meldung.xml`, `rmeld check antwort.xml` (verdict + summary, exit codes mirror rosci: 0 ok / 2 usage / 3 neutral answer / 4 fachliche rejection / 5 technical fault / 6 internal), `rmeld validate`. `--json` output toggle like rosci.

**Phase E — docs & finish.** STANDARD-NOTES.md (message types, schema files+versions, taxonomy with element/code references, 602.1 correction, version + XTA decisions); README with the correction up top; final report to you stating explicitly what was verified against the standard vs. what rests on assumptions.

## 4. Definition of done (handoff phase 1, verbatim mapping)

- [ ] rmeld-built 0600 passes official XSD validation (xmllint, offline, CI-enforced)
- [ ] Every 0601 response variant + technical faults parsed, typed, classified, fixture-tested
- [ ] Umlaute byte-exact through create→parse
- [ ] `cargo test` green offline; CI green; PROVENANCE.md + STANDARD-NOTES.md complete
- [ ] Honest summary: verified vs. assumed

## 5. Risks / open assumptions to re-check during work

- libxml2 ≥ 2.13 may reject the 26.11 schemas at compile time (patch + document, rosci precedent).
- The „gültig ab" schedule ≠ what individual intermediaries actually run; parser namespace-tolerance is the defense.
- Official 26.11 testsuite may not include simple-Auskunft examples → synthetic fixtures, marked as such.
- Subject/Betreff convention for the intermediary: unknown, recorded as open contract item.
