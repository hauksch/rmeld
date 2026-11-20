# rmeld — agent handoff brief

> **Status: historical.** This is the briefing the project started
> from, kept verbatim as a record. Its working assumptions were
> corrected during implementation — most notably the message number
> („602.1" does not exist; the implemented pair is 0600/0601) — see
> docs/STANDARD-NOTES.md for the verified facts. The rmeld Handoff brief
> is not maintained.

You are taking over an **empty repository**. Nothing is implemented, by
design. This file is the complete briefing; the human has stepped away.
Read it fully before writing any code.

## Mission

`rmeld` is a utility for the **fachlich (application) layer** of German
melderegister communication:

1. **Create** xMeld **602.1 „Einfache Melderegisterauskunft"** request
   messages from structured input (person search parameters) as valid
   xMeld XML.
2. **Parse** the Auskunft response messages that come back (byte-exact
   XML) into structured results.
3. **Classify** responses into result categories (fulfilled answer,
   negative answer, technical rejection, partial/qualified answer …).
4. **Validate** both directions against the official XSDs.

**Scope discipline: 602.1 only, for now.** Do not build generic xMeld
plumbing "for later"; do not touch transport (see next section). A
narrow tool that is provably correct beats a framework that is
maybe-correct.

## Division of labor — do NOT reimplement transport

Transport exists and is battle-tested. Your payload is handed over as
**opaque bytes**:

- `/home/hauke/Projects/osci-deshittifier` — **rosci**: OSCI-Transport
  1.2 stack (CLI `rosci` + REST server `roscid`). Its stated philosophy:
  *"The XTA file is treated as opaque bytes. You worry about the
  content; rosci worries about the ceremony."* Read `docs/OPERATIONS.md`
  (the business-flow runbook), `docs/REST-API.md` + `docs/openapi.yaml`
  (the REST contract), `docs/STANDARD-COMPLIANCE.md` (transport
  coverage).
- `/home/hauke/Projects/osci-rust-rewrite` — **oscirs**: the native
  OSCI implementation rosci builds on. Read-only context; you should
  never need it.

The pipeline you are the middle of: Fachanwendung → **rmeld (create
602.1)** → XTA file → rosci (sign/encrypt/transport) → Meldebehörde →
rosci (fetch/decrypt) → **rmeld (parse/classify/validate)** →
Fachanwendung.

Integration shape today: the created message goes out as a file (e.g.
`meldung.xta`) through `POST /v1/send` (base64 body) or the CLI; answers
arrive as base64 content parts from `POST /v1/fetch`. So the minimal
public shape of rmeld is roughly: structured input → XML file (create),
XML file → structured result + verdict (parse/classify/validate),
usable standalone **and** embeddable as a library.

## Domain primer — and what you must verify first

xMeld is the XÖV standard for Nachrichten an die Melderegister,
published by FITKO on **xoev.de** together with its dependencies
(XJustiz `basisdatentypen`, `xdienst` etc.). Versions come as
"Transferschema" packages containing the full XSD set + standard
documentation (docx/pdf).

**The user's working assumption — verify before trusting:** message
type **602.1** = „Einfache Melderegisterauskunft". The exact
Nachrichtennummer, its official name, its schema elements, and the
possible **response variants** (positive answer, „keine
Personenidentifikation" negative answer, technical fault messages,
partial answers) must be derived from the official schema set and the
standard's documentation — not from memory, not from blog posts. If
602.1 turns out to be the wrong number for the simple register request,
find the right one, note the correction prominently in the README, and
tell the human in your final report.

Deliver the verification as `docs/STANDARD-NOTES.md`: message types
involved, schema files and versions used, the classification taxonomy
you derived, with file/element references into the vendored XSDs.

## Recommended stack (justify or challenge, don't dither)

- **Rust**, edition 2021+, workspace layout like the sibling repos
  (`crates/rmeld` library + `crates/rmeld-cli` thin CLI). The ecosystem
  around this project is Rust; XML handling via `quick-xml`/`xsd`-style
  codegen or hand-rolled serde against the XSD — your call, but the
  schema is large, so codegen deserves a look before hand-rolling.
- Pinned toolchain via `rust-toolchain.toml` (the siblings pin
  `1.98.1`; match it so builds are reproducible across machines).
- Vendored schemas **with a `schema/PROVENANCE.md`** (the rosci pattern:
  exact upstream source, tag/version, retrieval date, and every local
  deviation documented). Never check in schemas without saying where
  they came from. Prefer vendoring over crates.io XML-schema crates.
- Validation must run **offline**: XSD validation via vendored schemas
  (e.g. `xmllint` invocation or a Rust validator crate). Test fixtures
  are local files; no network in `cargo test`, ever.

## Pitfalls learned the hard way next door (copy the defenses)

The rosci repo's CI history is a museum of exactly the failures you
should design against. Its `.github/workflows/` are the house CI
template — copy the shape (fmt check, clippy `-D warnings`, tests,
cargo-deny supply-chain audit), but know the traps:

1. **SHA-pinned GitHub actions lose input defaults.** If you pin
   `dtolnay/rust-toolchain` to a commit SHA, you MUST pass
   `with: toolchain: stable` — the ref-name default (`@stable`) is gone.
2. **Modern libxml2 (≥ 2.13) rejects non-deterministic content models**
   in old XÖV-era schemas at *schema compile* time. If your validation
   path uses xmllint, test it against a current libxml2 (the runner
   images ship 2.15) — "works with 2.9" is not proof. If vendored
   schemas need patching to compile, document each deviation in
   `PROVENANCE.md` (see rosci `schema/PROVENANCE.md` for the format).
3. **Tests must not silently skip.** A gate that skips when a tool is
   missing (rosci's old mistake: xmllint missing locally → test green
   locally, red in CI) hides exactly the failures you care about. Make
   missing-tool a loud failure or install the tool in CI.
4. Every action pinned by SHA; every dependency pinned by lockfile;
   `cargo-deny` over the workspace (licenses/advisories/sources) like
   the siblings.

## Open questions to resolve early (in this order)

1. **The 602.1 verification** (above) — everything downstream depends
   on the right message type and its response variants.
2. **Which xMeld version** do the real Meldebehörden/intermediaries run?
   Vendored schemas should match the version in actual use, not the
   newest published one. Ask the human if unclear after checking what
   rosci's operational docs assume.
3. **XTA envelope + subject convention.** rosci transports an `<XTA>`
   root-element file and warns (but sends) if the root is missing; the
   subject line convention (e.g. „XTA …"-style Betreff) is a contract
   with the intermediary, not with rosci. Decide where the XTA wrapping
   lives — inside rmeld's create output is the natural place — and
   record the decision.
4. **Encoding**: xMeld is UTF-8, but real-world payloads and filenames
   carry Umlaute; round-trip tests must cover ÄÖÜ/ß end-to-end.
5. **Test data**: build fixtures from the official xMeld demo/example
   messages if published; otherwise synthesize and mark them clearly as
   synthetic. No real Personenstand data, ever.

## Suggested first steps

1. `docs/STANDARD-NOTES.md` — download the xMeld Transfer package,
   verify 602.1, derive the create/parse/classify surface from the
   schemas. No app code yet.
2. Vendor the schema set + write `schema/PROVENANCE.md`; get an
   offline validation path working on a sample document (this is where
   pitfall 2 will bite — good, find out now).
3. `crates/rmeld` API skeleton: `Request` builder → XML; `Response`
   parse → `Classification` enum + structured result; both round-trip
   through fixtures.
4. CI workflow (copy rosci's shape, avoid its traps), rust-toolchain
   pin, cargo-deny.
5. CLI: `rmeld create … > meldung.xta`, `rmeld check antwort.xml`,
   designed to sit next to rosci's CLI in the pipeline.

## Definition of done (phase 1)

- A 602.1 request built by rmeld passes the official XSD validation.
- Every response variant the standard defines for 602.1 is parsed into
  a typed result, classified into the taxonomy, and covered by a
  fixture test (including the negative and technical-fault answers).
- Umlaute survive a create→parse round trip byte-exact.
- `cargo test` green offline; CI green on GitHub; schemas documented in
  PROVENANCE.md; STANDARD-NOTES.md answers the verification checklist.
- The human gets a summary that states explicitly what was verified
  against the standard and what still rests on assumptions.

## House conventions (from the sibling repos)

Conventional-commit style subjects with opinionated bodies; English
prose, German Fachbegriffe quoted („Einfache Melderegisterauskunft");
dry, honest comments that state constraints rather than narrate code;
fail-fast validation over half-working states; zero tolerance for
secrets or real personal data in the repo.
