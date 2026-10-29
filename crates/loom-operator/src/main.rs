#![forbid(unsafe_code)]

//! `b10x-loom-operator`: runs the controller, renders one resource's children, or prints the CRD.

use std::fs;
use std::io::Write as _;
use std::process::ExitCode;

use b10x_loom_operator::cli::{Cli, Command, RenderArgs, RunArgs};
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use b10x_loom_operator::{CRD, reconcile};
use clap::Parser;

fn failed(code: u8, why: &str) -> ExitCode {
    eprintln!("b10x-loom-operator: {why}");
    ExitCode::from(code)
}

fn print(text: &str) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failed(1, &format!("standard output: {error}")),
    }
}

fn render(arguments: &RenderArgs) -> ExitCode {
    let path = &arguments.resource;
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => return failed(1, &format!("--resource {}: {error}", path.display())),
    };
    let config = match &arguments.config {
        Some(file) => match Config::load(file) {
            Ok(config) => config,
            Err(error) => return failed(1, &error.0),
        },
        None => Config::builtin(),
    };
    match render_document(&text, &config) {
        Ok(children) => print(&children),
        Err(error @ RenderError::NotALoomAgent(_)) => {
            failed(2, &format!("--resource {}: {error}", path.display()))
        }
        Err(error @ RenderError::Catalog(_)) => failed(1, &error.to_string()),
        Err(error @ RenderError::Refused(_)) => failed(3, &error.to_string()),
    }
}

fn run(arguments: &RunArgs) -> ExitCode {
    // kube builds its TLS client on rustls' process-level provider. This binary's graph enables
    // both `ring` (kube, reqwest) and `aws-lc-rs` (`b10x-substrate-host`), so rustls cannot choose
    // one and panics at the first client: install `ring`, the provider kube's `rustls-tls` names.
    // An `Err` means a provider is installed already, which is all the client needs.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let config = match Config::load(&arguments.config) {
        Ok(config) => config,
        Err(error) => return failed(1, &error.0),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return failed(1, &format!("the runtime: {error}")),
    };
    runtime.block_on(async {
        let client = match kube::Client::try_default().await {
            Ok(client) => client,
            Err(error) => return failed(1, &format!("the cluster: {error}")),
        };
        match reconcile::run(client, config).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(why) => failed(1, &format!("the plugin catalog: {why}")),
        }
    })
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run(arguments) => run(&arguments),
        Command::Render(arguments) => render(&arguments),
        Command::Crd => print(CRD),
    }
}
