use clap::Parser;
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    suite: PathBuf,
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    diagnostics: PathBuf,
}

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args = Args::parse();
    let executed = b10x_loom_commission_conformance::run_suite(&fs::read_to_string(args.suite)?)?;
    fs::write(args.report, &executed.report)?;
    fs::write(args.diagnostics, &executed.diagnostics)?;
    let report: serde_json::Value = serde_json::from_str(&executed.report)?;
    let counts = &report["counts"];
    println!("{counts}");
    let count = |name: &str| counts[name].as_u64().expect("report count");
    Ok(ExitCode::from(if count("failed") + count("error") > 0 {
        1
    } else if count("passed") == 0 || count("skipped") + count("unsupported") > 0 {
        3
    } else {
        0
    }))
}
