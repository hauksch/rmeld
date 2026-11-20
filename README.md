# rmeld

> **`rmeld` — the fachlich half of a Melderegister conversation.**
> rosci moves the bytes; rmeld speaks the message.

[![ci](https://github.com/hauksch/rmeld/actions/workflows/ci.yml/badge.svg)](https://github.com/hauksch/rmeld/actions/workflows/ci.yml)
[![audit](https://github.com/hauksch/rmeld/actions/workflows/audit.yml/badge.svg)](https://github.com/hauksch/rmeld/actions/workflows/audit.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

`rmeld` creates, parses, classifies and XSD-validates the xMeld messages
of the „**Einfache Melderegisterauskunft**“ — the German melderegister's
simple person search — between a Fachanwendung and the OSCI transport
stack ([rosci](https://github.com/hauksch/rosci)). Payloads cross rosci
as opaque bytes; rmeld owns exactly the xMeld XML.

## A note on the message number

The project brief asked for „xMeld 602.1“; no such message exists in
any xMeld release. The implemented pair is **Nachricht 0600**
(„Anfrage zur einfachen Melderegisterauskunft“) and **Nachricht 0601**
(„Antwort zur einfachen Melderegisterauskunft“) — verified against the
vendored schemas and the official specification. The full correction
record lives in
[docs/STANDARD-NOTES.md](docs/STANDARD-NOTES.md), which also carries
every other standard fact this crate encodes, with sources and the
honest list of remaining assumptions.

## Why?

Because the fachlich layer is where correctness is legally load-bearing
(§ 44 ff. BMG), and because the wire is nastier than it looks: xMeld
mixes qualified structural elements with unqualified leaf data and
version-per-release namespaces. rmeld's answer is narrow and provable —
**every message it builds must pass the official XSDs, and every
response variant it parses is fixture-tested against them**, offline.
Codegen was tried first and produced non-compiling output
([the record](docs/STANDARD-NOTES.md)); hand-rolled serialization behind
an XSD gate it is.

## Install / build

```shell
make build    # cargo build --release (toolchain pinned to 1.98.1)
make test     # fmt + clippy -D warnings + test + cargo-deny
```

Requirements: Rust 1.98.1 (via rust-toolchain.toml), xmllint
(`apt-get install libxml2-utils` — the validation path fails loudly
without it, by design), no network.

## Usage

```shell
# Build a 0600 request from structured JSON, XSD-proven, wrapped in the
# <XTA> transport envelope (rosci-ready):
rmeld create anfrage.json > meldung.xta
rmeld create anfrage.json --betreff "XMeld 26.11 Melderegisterauskunft" > meldung.xta

# Read a 0601 answer (or RTS 0900/0910): verdict on stdout, exit code carries it
rmeld check antwort.xta
rmeld check antwort.xta --json      # structured result instead of prose

# Offline schema gate against the vendored XMeld 26.11 schemas:
rmeld validate meldung.xta
```

Exit codes: `0` fulfilled/ok · `2` usage, invalid input or schema
violations · `3` neutral answer („keine Personenidentifikation“) ·
`4` fachlich rejected (Nichterstellung / antwortstatus 03, 04) ·
`5` technical fault (antwortstatus 01/05, RTS) · `6` internal.
Shell scripts may branch on the verdict without parsing prose.

The JSON input contract (`create`) and every encoded standard fact are
documented in docs/STANDARD-NOTES.md.

## Architecture

```text
┌───────────────┐   0600 XML   ┌────────┐  XTA file  ┌─────────┐
│ Fachanwendung │─────────────▶│ rmeld  │───────────▶│  rosci  │──▶ Meldebehörde
│               │◀─────────────│        │◀───────────│         │
└───────────────┘   0601/RTS   └────────┘  XTA file  └─────────┘
                  classify+verdict
```

- `crates/rmeld` — library: `create` (JSON → 0600), `parse` (0601,
  RTS 0900/0910, 0600 echo), `classification`
  (Fulfilled/Neutral/Rejected/TechnicalFault with the official code
  texts), `validate` (xmllint + catalog, schemas embedded in the
  binary).
- `crates/rmeld-cli` — the `rmeld` binary above.
- `schema/` — vendored XMeld 26.11 + XInneres + XÖV schemas, OASIS
  catalog for offline resolution, PROVENANCE.md + SHA256SUMS.

## Testing

- `cargo test` runs fully offline; the XSD gate validates every fixture
  and every rmeld-built message against the official 26.11 schemas.
- The official KoSIT reference messages (FachspezifischEMRA001–006,
  XMeld 27.05 namespace) are vendored under `tests/fixtures/referenz/`
  and parsed/classified — proof against non-synthetic input.
- Umlauts survive create→parse byte-exact; ÄÖÜ/ß are in the fixtures on
  purpose. (So is the lesson that DIN 91379 rejects the en-dash.)
- Live intermediary tests do not exist; nothing here has spoken to a
  real Meldebehörde yet.

## Documentation

- [docs/STANDARD-NOTES.md](docs/STANDARD-NOTES.md) — the standard facts
  rmeld encodes (message types, wire rules, code lists, taxonomy), the
  verification record, and the honest list of remaining assumptions.
- [schema/PROVENANCE.md](schema/PROVENANCE.md) — where the vendored
  schemas come from, with checksums.
- [docs/HANDOFF.md](docs/HANDOFF.md) — the historical briefing the
  project started from (its premises are corrected in
  STANDARD-NOTES.md).
- [crates/rmeld/tests/FIXTURES.md](crates/rmeld/tests/FIXTURES.md) —
  what the fixtures are and where they come from.

## Ground rules

1. Scope discipline: 0600/0601 (+ RTS as the technical-fault vehicle)
   only — no generic xMeld plumbing „for later“.
2. Every standard fact cites the vendored schemas; assumptions are
   listed as assumptions (docs/STANDARD-NOTES.md §8), never disguised as
   facts.
3. Fail fast: a validation that silently skips is a bug, missing xmllint
   is a loud error, unknown codes stay raw instead of being guessed.
4. No real Personenstand data, ever; fixtures are synthetic or official
   KoSIT test messages (fictional persons by construction).
5. Comments may grumble about bureaucracy. Gently.

## Ecosystem

- [rosci](https://github.com/hauksch/rosci) — the OSCI-Transport 1.2
  stack that moves rmeld's payloads as opaque bytes.
- [oscirs](https://github.com/hauksch/oscirs) — the native OSCI
  implementation underneath.

## License

MIT — see [LICENSE](LICENSE).
