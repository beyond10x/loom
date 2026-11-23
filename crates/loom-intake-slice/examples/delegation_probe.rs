//! The half of `loom-xtask delegation-probe` that runs inside a delegated scope of the user's
//! systemd manager (`systemd-run --user --scope -p Delegate=yes`), as the delegated tests' harness
//! does (`tests/support/mod.rs`): it moves itself into a leaf of the scope
//! ([`prepare_delegated_cgroup`]) and opens Substrate's driver on the scope for an empty workspace
//! ([`SubstrateRunner::preflight`]). It prints `served` and exits 0 when the driver serves every
//! family Loom needs, else prints what refused and exits 1.

use std::process::ExitCode;

use b10x_loom_intake_slice::confinement::{SubstrateRunner, prepare_delegated_cgroup};

fn main() -> ExitCode {
    let cgroup = match prepare_delegated_cgroup() {
        Ok(cgroup) => cgroup,
        Err(error) => {
            println!("refused: a delegated cgroup v2 scope ({error})");
            return ExitCode::FAILURE;
        }
    };
    let fixture = match tempfile::tempdir() {
        Ok(fixture) => fixture,
        Err(error) => {
            println!("refused: a scratch directory ({error})");
            return ExitCode::FAILURE;
        }
    };
    let workspace = fixture.path().join("workspace");
    if let Err(error) = std::fs::create_dir(&workspace) {
        println!("refused: a workspace {} ({error})", workspace.display());
        return ExitCode::FAILURE;
    }
    match SubstrateRunner::new(Some(cgroup)).preflight(&workspace) {
        Ok(()) => {
            println!("served");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("refused: the Substrate driver ({error})");
            ExitCode::FAILURE
        }
    }
}
