//! Offline XSD validation via xmllint and the vendored XMeld 26.11 schemas.
//!
//! The schemas are embedded ([`include_str!`]) and materialized once into
//! a process-lifetime temp directory, together with the OASIS catalog —
//! so validation works from any working directory, never touches the
//! network, and `cargo test` needs no path setup. libxml2 ≥ 2.13 does not
//! fetch remote URLs at all, which makes the catalog load-bearing.
//!
//! xmllint is the one external tool this crate needs. Missing-tool is a
//! loud failure ([`crate::Error::XmllintUnavailable`]), never a silent
//! skip — a green validation that didn't run is exactly the failure mode
//! the rosci history teaches us to design against. Lookup order:
//! `RMELD_XMLLINT` env, `PATH`, `~/.local/bin/xmllint`. CI installs
//! libxml2-utils, so the gate is always armed there.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use tempfile::TempDir;

use crate::error::Error;

/// Which schema set a document validates against; derived from the root
/// element unless given explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    /// xmeld-nachrichten-melderegisterauskunft.xsd (0600, 0601).
    Melderegisterauskunft,
    /// xmeld-nachrichten-administration.xsd (RTS 0900/0910).
    Administration,
}

impl SchemaKind {
    fn schema_file(&self) -> &'static str {
        match self {
            SchemaKind::Melderegisterauskunft => {
                "xmeld/xmeld-nachrichten-melderegisterauskunft.xsd"
            }
            SchemaKind::Administration => "xmeld/xmeld-nachrichten-administration.xsd",
        }
    }
}

/// The embedded schema tree, mirroring the repo's `schema/` layout
/// (relative includes resolve inside it; the catalog resolves the
/// absolute import URLs). Keep in sync with schema/SHA256SUMS.
const SCHEMA_FILES: &[(&str, &str)] = &[
    ("catalog.xml", include_str!("../../../schema/catalog.xml")),
    // xmeld compile units
    (
        "xmeld/xmeld-nachrichten-melderegisterauskunft.xsd",
        include_str!("../../../schema/xmeld/xmeld-nachrichten-melderegisterauskunft.xsd"),
    ),
    (
        "xmeld/xmeld-nachrichten-administration.xsd",
        include_str!("../../../schema/xmeld/xmeld-nachrichten-administration.xsd"),
    ),
    (
        "xmeld/xmeld-nachrichten-basistypen.xsd",
        include_str!("../../../schema/xmeld/xmeld-nachrichten-basistypen.xsd"),
    ),
    (
        "xmeld/xmeld-baukasten.xsd",
        include_str!("../../../schema/xmeld/xmeld-baukasten.xsd"),
    ),
    (
        "xmeld/xmeld-basistypen.xsd",
        include_str!("../../../schema/xmeld/xmeld-basistypen.xsd"),
    ),
    (
        "xmeld/xmeld-codes.xsd",
        include_str!("../../../schema/xmeld/xmeld-codes.xsd"),
    ),
    // xinneres modules
    (
        "xinneres/xinneres-name.xsd",
        include_str!("../../../schema/xinneres/xinneres-name.xsd"),
    ),
    (
        "xinneres/xinneres-auslandsanschrift.xsd",
        include_str!("../../../schema/xinneres/xinneres-auslandsanschrift.xsd"),
    ),
    (
        "xinneres/xinneres-basisnachricht.xsd",
        include_str!("../../../schema/xinneres/xinneres-basisnachricht.xsd"),
    ),
    (
        "xinneres/xinneres-behoerde.xsd",
        include_str!("../../../schema/xinneres/xinneres-behoerde.xsd"),
    ),
    (
        "xinneres/xinneres-codes-gemeindeverzeichnis.xsd",
        include_str!("../../../schema/xinneres/xinneres-codes-gemeindeverzeichnis.xsd"),
    ),
    (
        "xinneres/xinneres-datum.xsd",
        include_str!("../../../schema/xinneres/xinneres-datum.xsd"),
    ),
    (
        "xinneres/xinneres-geschlecht.xsd",
        include_str!("../../../schema/xinneres/xinneres-geschlecht.xsd"),
    ),
    (
        "xinneres/xinneres-idnr.xsd",
        include_str!("../../../schema/xinneres/xinneres-idnr.xsd"),
    ),
    (
        "xinneres/xinneres-meldeanschrift.xsd",
        include_str!("../../../schema/xinneres/xinneres-meldeanschrift.xsd"),
    ),
    (
        "xinneres/xinneres-nachrichtentypen.xsd",
        include_str!("../../../schema/xinneres/xinneres-nachrichtentypen.xsd"),
    ),
    (
        "xinneres/xinneres-postanschrift.xsd",
        include_str!("../../../schema/xinneres/xinneres-postanschrift.xsd"),
    ),
    (
        "xinneres/xinneres-rueckweisung.xsd",
        include_str!("../../../schema/xinneres/xinneres-rueckweisung.xsd"),
    ),
    // xoev shared schemata
    (
        "xoev/din-norm-91379-datatypes.xsd",
        include_str!("../../../schema/xoev/din-norm-91379-datatypes.xsd"),
    ),
    (
        "xoev/xoev-basisnachricht-unqualified-g2g_1.1.xsd",
        include_str!("../../../schema/xoev/xoev-basisnachricht-unqualified-g2g_1.1.xsd"),
    ),
    (
        "xoev/xoev-code.xsd",
        include_str!("../../../schema/xoev/xoev-code.xsd"),
    ),
];

static SCHEMA_DIR: OnceLock<Result<PathBuf, String>> = OnceLock::new();
static XMLLINT: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Materialize the embedded schema tree once; keep the temp dir for the
/// process lifetime (dropping it would delete the schemas under xmllint's
/// feet on every later call).
fn schema_dir() -> Result<&'static Path, Error> {
    let init = || -> Result<PathBuf, String> {
        let dir = TempDir::with_prefix("rmeld-schemas-")
            .map_err(|e| format!("cannot create temp dir: {e}"))?
            .keep();
        for (rel, content) in SCHEMA_FILES {
            let target = dir.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
            }
            std::fs::write(&target, content)
                .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        }
        Ok(dir)
    };
    match SCHEMA_DIR.get_or_init(init) {
        Ok(path) => Ok(path.as_path()),
        Err(msg) => Err(Error::Io(std::io::Error::other(msg.clone()))),
    }
}

/// Locate xmllint: RMELD_XMLLINT, PATH, ~/.local/bin. Cached; every
/// failure is loud with the exact reason.
fn xmllint() -> Result<&'static Path, Error> {
    let init = || -> Result<PathBuf, String> {
        let candidates: Vec<PathBuf> = env::var_os("RMELD_XMLLINT")
            .map(|v| vec![PathBuf::from(v)])
            .unwrap_or_else(|| {
                let mut v: Vec<PathBuf> = env::var_os("PATH")
                    .map(|path| {
                        env::split_paths(&path)
                            .map(|dir| dir.join("xmllint"))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(home) = env::var_os("HOME") {
                    v.push(
                        PathBuf::from(home)
                            .join(".local")
                            .join("bin")
                            .join("xmllint"),
                    );
                }
                v
            });
        for candidate in candidates {
            if is_working_xmllint(&candidate) {
                return Ok(candidate);
            }
        }
        Err(
            "no working xmllint found — install libxml2-utils or point RMELD_XMLLINT at the binary. \
             Validation does not silently skip (see docs/STANDARD-NOTES.md)."
                .to_string(),
        )
    };
    match XMLLINT.get_or_init(init) {
        Ok(path) => Ok(path.as_path()),
        Err(msg) => Err(Error::XmllintUnavailable {
            path: PathBuf::from("RMELD_XMLLINT|PATH|~/.local/bin"),
            reason: msg.clone(),
        }),
    }
}

/// The candidate is usable if it spawns at all — xmllint prints its
/// version to stderr either way; deeper breakage surfaces in `validate`
/// with the exact invocation error.
fn is_working_xmllint(path: &Path) -> bool {
    Command::new(path).arg("--version").output().is_ok()
}

/// Validate an xMeld message against the official schema for its root
/// element. Returns `Ok(())` or [`Error::SchemaViolations`] with xmllint's
/// report verbatim.
pub fn validate(instance: &[u8]) -> Result<(), Error> {
    let (kind, instance) = sniff_and_unwrap(instance)?;
    validate_as(&instance, kind)
}

/// Validate against a specific schema set, regardless of the root element.
pub fn validate_as(instance: &[u8], kind: SchemaKind) -> Result<(), Error> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(0);

    let xmllint = xmllint()?;
    let dir = schema_dir()?;
    // Parallel callers share the schema dir; a fixed filename would race
    // one test's xmllint against another's fs::write.
    let n = INSTANCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let instance_path = dir.join(format!("instance-{n}.xml"));
    std::fs::write(&instance_path, instance)?;

    let mut command = Command::new(xmllint);
    command
        .arg("--noout")
        .arg("--nonet")
        .arg("--schema")
        .arg(dir.join(kind.schema_file()))
        .arg(&instance_path)
        .env("XML_CATALOG_FILES", dir.join("catalog.xml"))
        .current_dir(dir);

    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::SchemaViolations(stderr.trim().to_string()))
}

/// Sniff the root element and pick the schema set; an `<XTA>` envelope
/// root (the transport-file convention, case-insensitive like rosci's
/// sniff) is unwrapped: the returned instance bytes are the fachlich
/// message inside, ready for xmllint (the envelope itself has no schema).
fn sniff_and_unwrap(instance: &[u8]) -> Result<(SchemaKind, Vec<u8>), Error> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(instance);
    reader.config_mut().trim_text(true);
    let mut in_envelope = false;
    loop {
        let pos_before_read = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|e| Error::Malformed(format!("xml error: {e}")))?;
        match event {
            Event::Start(start) => {
                let root = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
                if in_envelope {
                    // Inner message root: pos_before_read is the element's
                    // start offset; the span walker gives the matching
                    // End — the message bytes in between are the instance.
                    let span = inner_element_span(instance, pos_before_read)?;
                    let instance = instance[span.0..span.1].to_vec();
                    return match root.as_str() {
                        "melderegisterauskunfteinfach.anforderung.0600"
                        | "melderegisterauskunfteinfach.antwort.0601" => {
                            Ok((SchemaKind::Melderegisterauskunft, instance))
                        }
                        "administration.returntosender.0900"
                        | "administration.returntosender.0910" => {
                            Ok((SchemaKind::Administration, instance))
                        }
                        other => Err(Error::UnknownMessage {
                            root: other.to_string(),
                            namespace: "(unknown)".to_string(),
                        }),
                    };
                }
                if root.eq_ignore_ascii_case("XTA") {
                    in_envelope = true;
                    continue;
                }
                return match root.as_str() {
                    "melderegisterauskunfteinfach.anforderung.0600"
                    | "melderegisterauskunfteinfach.antwort.0601" => {
                        Ok((SchemaKind::Melderegisterauskunft, instance.to_vec()))
                    }
                    "administration.returntosender.0900" | "administration.returntosender.0910" => {
                        Ok((SchemaKind::Administration, instance.to_vec()))
                    }
                    other => Err(Error::UnknownMessage {
                        root: other.to_string(),
                        namespace: "(unknown)".to_string(),
                    }),
                };
            }
            Event::Eof => {
                return Err(Error::Malformed("no root element found".to_string()));
            }
            _ => {}
        }
    }
}

/// The inner message element spans from the Start event at `start_pos`
/// (the reader position just before that Start was consumed) to its
/// matching End. Walks the events from `start_pos` tracking depth and
/// returns the end offset of the matching End event.
fn inner_element_span(instance: &[u8], start_pos: usize) -> Result<(usize, usize), Error> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(&instance[start_pos..]);
    reader.config_mut().trim_text(true);
    let mut depth = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| Error::Malformed(format!("xml error: {e}")))?;
        match event {
            Event::Start(_) => depth += 1,
            Event::Empty(_) => {}
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start_pos + reader.buffer_position() as usize;
                    return Ok((start_pos, end));
                }
            }
            Event::Eof => {
                return Err(Error::Malformed(
                    "XTA envelope: inner message element is not closed".to_string(),
                ));
            }
            _ => {}
        }
    }
}
