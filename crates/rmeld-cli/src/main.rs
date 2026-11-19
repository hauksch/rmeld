//! `rmeld` — create/check for the „Einfache Melderegisterauskunft“ message
//! family. Designed to sit next to rosci's CLI in the pipeline:
//!
//! ```text
//! Fachanwendung → rmeld create anfrage.json > meldung.xta
//!                                        │
//!                                 rosci (OSCI)
//!                                        │
//!               Fachanwendung ← rmeld check antwort.xml
//! ```
//!
//! Exit codes mirror the rosci convention, extended with fachlich
//! verdicts: 0 ok · 2 usage/invalid input/validation failure · 3 neutral
//! answer · 4 fachlich rejection · 5 technical fault · 6 internal.

use std::io::Read;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rmeld::{Anfrage, Classification, Message};

mod cli_error;

use cli_error::Error;

/// Fachbegriffe quoted like the standard writes them; the tool speaks
/// plainly about the rest.
const ABOUT: &str = "rmeld — fachlich layer for xMeld 0600/0601 („Einfache
Melderegisterauskunft“)

Exit codes:
  0  ok (answer fulfilled, or validation passed)
  2  usage error, invalid input, or schema violations
  3  neutral answer („keine Personenidentifikation“)
  4  fachlich rejected (Nichterstellung / antwortstatus 03, 04)
  5  technical fault (antwortstatus 01/05, RTS 0900/0910)
  6  internal error (xmllint missing, IO, XML plumbing)";

#[derive(Debug, Parser)]
#[command(name = "rmeld", about = ABOUT, disable_help_subcommand = true, version)]
struct Cli {
    /// -v: warn, -vv: info, -vvv: debug (trace needs RMELD_LOG)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Emit machine-readable JSON instead of human text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Build a 0600 „Anfrage zur einfachen Melderegisterauskunft“ from a
    /// JSON input file (or stdin with `-`); XML goes to stdout.
    Create {
        /// JSON input file per the AnfrageInput contract (docs/STANDARD-NOTES.md)
        #[arg(value_name = "INPUT.json")]
        input: String,
        /// Betreff of the delivery (the free OSCI 1.2 Subject string).
        /// Intermediary convention — see docs/STANDARD-NOTES.md §7.
        /// Empty when not supplied.
        #[arg(long, value_name = "BETREFF", default_value = "")]
        betreff: String,
    },
    /// Parse a 0601 answer (or RTS 0900/0910), print the classification
    /// and the structured result; exit code carries the verdict.
    Check {
        /// xMeld XML file (or stdin with `-`)
        #[arg(value_name = "ANTWORT.xml")]
        message: String,
        /// Also XSD-validate before parsing (off by default: `check` is
        /// the fast fachlich read; `validate` is the schema gate)
        #[arg(long)]
        validate: bool,
    },
    /// XSD-validate an xMeld message against the vendored XMeld 26.11
    /// schemas, offline.
    Validate {
        /// xMeld XML file (or stdin with `-`)
        #[arg(value_name = "MESSAGE.xml")]
        message: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("rmeld: {err}");
            ExitCode::from(err.exit_code() as u8)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Error> {
    match cli.cmd {
        Cmd::Create { input, betreff } => cmd_create(&input, &betreff),
        Cmd::Check {
            message,
            validate: do_validate,
        } => cmd_check(&message, do_validate, cli.json),
        Cmd::Validate { message } => cmd_validate(&message),
    }
}

fn cmd_create(input: &str, betreff: &str) -> Result<ExitCode, Error> {
    let json = read_input(input)?;
    let parsed: rmeld::AnfrageInput = serde_json::from_slice(&json)
        .map_err(|e| Error::InvalidInput(format!("JSON contract: {e}")))?;
    let anfrage = Anfrage::build(parsed)?;
    // The XTA transport file: convention envelope, Betreff on the root.
    let xta = anfrage.to_xta(Some(betreff))?;
    // Create is fail-fast end to end: what leaves this tool is proven
    // against the official schema, not hopefully correct — the gate sees
    // through the envelope to the fachlich message.
    if let Err(e) = rmeld::validate(xta.as_bytes()) {
        return Err(Error::from_rmeld(e));
    }
    println!("{xta}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_check(message: &str, do_validate: bool, json: bool) -> Result<ExitCode, Error> {
    let bytes = read_input(message)?;
    if do_validate {
        rmeld::validate(&bytes).map_err(Error::from_rmeld)?;
    }
    let verdict = match Message::parse(&bytes).map_err(Error::from_rmeld)? {
        Message::Anfrage0600(a) => {
            print_anfrage_verdict(&a, json)?;
            return Ok(ExitCode::SUCCESS);
        }
        Message::Antwort0601(a) => a.classify(),
        Message::ReturnToSender(rts) => rts.classify(),
    };
    match &verdict {
        Classification::Fulfilled(e) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "classification": "fulfilled",
                        "ergebnisstatus": e.ergebnisstatus,
                        "zusatzinformationen": e.zusatzinformationen,
                        "namen": e.namen,
                        "anschriften": e.anschriften,
                    })
                );
            } else {
                println!(" fulfilled — {}", verdict.text());
                if let Some(n) = &e.namen {
                    if let Some(fam) = &n.familienname {
                        println!(" familienname: {fam}");
                    }
                    if let Some(v) = &n.vornamen {
                        println!(" vornamen: {v}");
                    }
                }
                for a in &e.anschriften {
                    println!(
                        " anschrift: {} {} {}, {}",
                        a.strasse.clone().unwrap_or_default(),
                        a.hausnummer.clone().unwrap_or_default(),
                        a.wohnort.clone().unwrap_or_default(),
                        a.postleitzahl.clone().unwrap_or_default(),
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Classification::Neutral => {
            if json {
                println!("{}", serde_json::json!({"classification": "neutral"}));
            } else {
                println!(" neutral — {}", verdict.text());
            }
            Ok(ExitCode::from(3))
        }
        Classification::Rejected(r) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "classification": "rejected",
                        "quelle": format!("{:?}", r.quelle),
                        "code": r.code,
                    })
                );
            } else {
                println!(" rejected — {}", verdict.text());
            }
            Ok(ExitCode::from(4))
        }
        Classification::TechnicalFault(f) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "classification": "technical-fault",
                        "quelle": format!("{:?}", f.quelle),
                        "code": f.code,
                    })
                );
            } else {
                println!(" technical fault — {}", verdict.text());
            }
            Ok(ExitCode::from(5))
        }
    }
}

fn print_anfrage_verdict(anfrage: &rmeld::AnfrageEcho, json: bool) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "message": "0600 anfrage",
                "zeichennachricht": anfrage.zeichennachricht,
                "einzelfaelle": anfrage.einzelfaelle,
            })
        );
    } else {
        println!(
            " 0600 anfrage, {} Einzelanfrage(n), zeichennachricht {:?}",
            anfrage.einzelfaelle.len(),
            anfrage.zeichennachricht,
        );
    }
    Ok(())
}

fn cmd_validate(message: &str) -> Result<ExitCode, Error> {
    let bytes = read_input(message)?;
    rmeld::validate(&bytes).map_err(Error::from_rmeld)?;
    println!(" validates against the vendored XMeld 26.11 schemas");
    Ok(ExitCode::SUCCESS)
}

/// Read the argument (`-` or a path) fully.
fn read_input(arg: &str) -> Result<Vec<u8>, Error> {
    if arg == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| Error::InvalidInput(format!("stdin: {e}")))?;
        Ok(buf)
    } else {
        std::fs::read(arg).map_err(|e| Error::InvalidInput(format!("{}: {e}", arg)))
    }
}

/// -v/-vv/-vvv → warn/info/debug; RMELD_LOG env overrides, stderr only.
fn init_tracing(verbose: u8) {
    use tracing_subscriber::EnvFilter;
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("rmeld={level},rmeld_cli={level}")));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
