# Schema provenance

Vendored for the **xMeld 26.11** release (Fassung 2026-01-31, gültig ab
2026-11-01 — the binding release since 2026-11-01), retrieved **2026-11-16**
via `scripts/vendor-schemas.sh`, checksums in `schema/SHA256SUMS`.

Three upstream families, none of them modified:

- `schema/xmeld/` — the 20 XSDs of the XMeld 26.11 Transferschema, from
  [osci.de/xmeld2611](https://osci.de/xmeld2611/) (KoSIT's publish mirror
  for the release; equivalent to the XRepository package
  `urn:xoev-de:kosit:standard:xinneres.xmeld_26.11`). Master schema
  `xinneres.xmeld.xsd`, target namespace
  `http://www.osci.de/xmeld2611` — the namespace changes with every
  release, so parsers must key on it.
- `schema/xinneres/` — the 12 imported XInneres-Basismodul schemas
  (name/4, auslandsanschrift/5, basisnachricht/7, behörde/7,
  gemeindeverzeichnis-codes/3, datum/2, geschlecht/1, idnr/2,
  meldeanschrift/5, nachrichtentypen/7, postanschrift/5, rueckweisung/6),
  from [osci.de/xinneres](https://www.osci.de/xinneres/). Flattened from
  their upstream `modul/version/` paths — the catalog (not the directory
  tree) maps the URLs as they appear in `xs:import`.
- `schema/xoev/` — shared XÖV schemata: DIN SPEC 91379 datatypes
  (2022-08), Basisnachricht unqualified g2g 1.1, xoev-code 1.0, from
  xoev.de.

XJustiz (`basisdatentypen`) and `xdienst` are **not** used by the
Melderegisterauskunft message family — the hand-off brief assumed
otherwise; the import closure above is complete.

`catalog.xml` (OASIS XML catalog) resolves every `xs:import`
`schemaLocation` URL to its vendored file so xmllint validates fully
offline. Relative `xs:include`s resolve inside each family directory.
libxml2 ≥ 2.13 does not fetch remote URLs at all, so the catalog is
mandatory, not an optimization.

**Deviation from the upstream files:** line endings only. Upstream serves
`xinneres-datum.xsd` and `xoev-code.xsd` with CRLF; both are normalized to
LF per repo policy (`.gitattributes`) — content bytes otherwise identical,
checksums taken after normalization. The XMeld 26.11 schemas compile as
shipped under libxml2 2.15 (deterministic content model enforcement
included) — unlike the OSCI 1.2 schemas next door, which needed two
documented patches. If a re-vendor ever requires a patch, it is applied
in place and documented here with file, line and rationale.

Used by `crates/rmeld`'s validation path (`rmeld validate` / the
`check` gate in tests) via xmllint + `XML_CATALOG_FILES=schema/catalog.xml`.
Re-vendor + re-verify checksums when bumping the xMeld release; the
parser (not the validator) is the place where other release namespaces
(http://www.osci.de/xmeld34, …xmeld2605, …xmeld2705) are tolerated.
