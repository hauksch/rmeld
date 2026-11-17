#!/usr/bin/env bash
# Re-vendor the schema/ directory from upstream. Manual maintenance tool —
# `cargo test` never runs this; the vendored files and SHA256SUMS are the
# source of truth inside the repo.
#
# Usage: scripts/vendor-schemas.sh   (from the repo root)
# After running: diff schema/SHA256SUMS against the freshly written
# schema/SHA256SUMS.new — byte-identical upstream files reproduce it; any
# drift (or an upstream re-publication) must be justified in PROVENANCE.md
# before the new checksums are committed.
set -euo pipefail

XMELD_BASE="https://www.osci.de/xmeld2611"
XINNERES_BASE="https://www.osci.de/xinneres"
XMELD_FILES=(
	xinneres.xmeld.xsd
	xmeld-basistypen.xsd
	xmeld-baukasten.xsd
	xmeld-codes.xsd
	xmeld-nachrichten-administration.xsd
	xmeld-nachrichten-anmeldung.xsd
	xmeld-nachrichten-auslaenderzentralregister.xsd
	xmeld-nachrichten-basistypen.xsd
	xmeld-nachrichten-datenabrufe.xsd
	xmeld-nachrichten-datenuebermittlung.xsd
	xmeld-nachrichten-ergbevstat.xsd
	xmeld-nachrichten-fortschreibung.xsd
	xmeld-nachrichten-hinweis.xsd
	xmeld-nachrichten-kirche.xsd
	xmeld-nachrichten-lra.xsd
	xmeld-nachrichten-melderegisterauskunft.xsd
	xmeld-nachrichten-onlinedienste.xsd
	xmeld-nachrichten-rueckmeldung.xsd
	xmeld-nachrichten-statistik.xsd
	xmeld-nachrichten-xmeldit.xsd
)
XINNERES_FILES=(
	allgemeinername/4/xinneres-name.xsd
	auslandsanschrift/5/xinneres-auslandsanschrift.xsd
	basisnachricht/7/xinneres-basisnachricht.xsd
	behoerde/7/xinneres-behoerde.xsd
	codes/gemeindeverzeichnis/3/xinneres-codes-gemeindeverzeichnis.xsd
	datum/2/xinneres-datum.xsd
	geschlecht/1/xinneres-geschlecht.xsd
	idnr/2/xinneres-idnr.xsd
	meldeanschrift/5/xinneres-meldeanschrift.xsd
	nachrichtentypen/7/xinneres-nachrichtentypen.xsd
	postanschrift/5/xinneres-postanschrift.xsd
	rueckweisung/6/xinneres-rueckweisung.xsd
)

fetch() { curl -fsSL -o "$2" "$1" && echo "fetched $2"; }

for f in "${XMELD_FILES[@]}"; do fetch "$XMELD_BASE/$f" "schema/xmeld/$f"; done
for f in "${XINNERES_FILES[@]}"; do fetch "$XINNERES_BASE/$f" "schema/xinneres/$(basename "$f")"; done
fetch "https://xoev.de/schemata/din/91379/2022-08/din-norm-91379-datatypes.xsd" "schema/xoev/din-norm-91379-datatypes.xsd"
fetch "http://xoev.de/schemata/basisnachricht/unqualified/g2g/1_1/xoev-basisnachricht-unqualified-g2g_1.1.xsd" "schema/xoev/xoev-basisnachricht-unqualified-g2g_1.1.xsd"
fetch "http://xoev.de/schemata/code/1_0/xoev-code.xsd" "schema/xoev/xoev-code.xsd"

(cd schema && find . -name '*.xsd' -exec sha256sum {} \; | sort -k2) > schema/SHA256SUMS.new
echo "compare schema/SHA256SUMS.new with schema/SHA256SUMS, then reconcile PROVENANCE.md"
