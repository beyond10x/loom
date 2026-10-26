//! The command line `b10x-loom-operator`: `run` the controller, `render` the children of one
//! resource, print the `crd`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// What `run` does and how it exits.
pub const RUN_HELP: &str =
    "The configuration names the one namespace watched, the default agent image and resources, the
model route a resource with empty routes is given, and the Secret names and egress hosts a resource
may use. The cluster is the in-cluster service account's, else the current kubeconfig context's.
Each resource is checked in-process before any child exists; a refused one has no children and its
status names the refusal. The record of the resources lives in memory: after a restart every
resource is checked again and its status rewritten.

Exit status:
  0  the watch ended on a signal
  1  the configuration, the plugin catalog or the cluster connection cannot be read
  2  the command line is not valid";

/// What `render` prints and how it exits.
pub const RENDER_HELP: &str = "The resource's namespace is its metadata.namespace, else the configuration's. Without --config the
built-in configuration applies: namespace default, the agent image registry.example/loom-agent at
this version, 250m CPU and 256Mi memory, no model route, and no Secret name or egress host admitted.
The children carry the resource as owner only when the document has a metadata.uid.

Exit status:
  0  the children, as one multi-document YAML on standard output
  3  refused: standard error carries \"refused: <reason> at <member>: <detail>\"
  1  a file cannot be read, or the plugin catalog cannot be loaded
  2  the command line is not valid, or the document is not a LoomAgent";

/// The `b10x-loom-operator` command line.
#[derive(Debug, Parser)]
#[command(
    name = "b10x-loom-operator",
    version,
    about = "The Kubernetes controller of the LoomAgent resource: each resource runs as one checked agent pod"
)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The commands of `b10x-loom-operator`.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Watch the LoomAgent resources of the configured namespace and reconcile each into its
    /// ServiceAccount, ConfigMap, Deployment, NetworkPolicy and Service, publishing its lifecycle
    /// as its status.
    #[command(after_help = RUN_HELP)]
    Run(RunArgs),
    /// Print the children of one LoomAgent document as one multi-document YAML, after the checks
    /// the controller runs. Nothing is applied and no cluster is read.
    #[command(after_help = RENDER_HELP)]
    Render(RenderArgs),
    /// Print the LoomAgent CustomResourceDefinition, the bytes of
    /// deploy/crd/loomagents.loom.beyond10x.dev.yaml.
    Crd,
}

/// The arguments of `b10x-loom-operator run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// The controller's configuration, YAML: namespace, defaultImage, defaultResources,
    /// modelRoute, allowedSecretNames, allowedEgressHosts, allowedClaimNames.
    #[arg(long, value_name = "FILE")]
    pub config: PathBuf,
}

/// The arguments of `b10x-loom-operator render`.
#[derive(Debug, Args)]
pub struct RenderArgs {
    /// One LoomAgent document, YAML or JSON.
    #[arg(long, value_name = "FILE")]
    pub resource: PathBuf,
    /// The controller's configuration, as run reads it; the built-in one without it.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_command_line_is_well_formed() {
        Cli::command().debug_assert();
    }
}
