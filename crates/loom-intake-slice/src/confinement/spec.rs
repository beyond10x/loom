//! A child's confinement as one value (`story:confinement-spec`): [`ConfinementSpec`], declared in
//! `intake.confinement`, applied by [`ChildCommand::confined`] through the builders, so a launch
//! checks a spec's command exactly as it checks the same builder calls.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

use super::{
    BaselineEnvironment, BaselineVariable, ChildAperture, ChildCommand, ChildEnvironment,
    ChildNetwork, ChildProfile, ConfinementError, ConfinementRefusal, ConfinementSpec,
    EgressAperture, ExecEnvironment, NetworkMode, SandboxProfile, refuse, scope_error,
};

impl ChildCommand {
    /// The same child confined as `spec` declares, each field applied through the builder of the
    /// same meaning, in this order: [`ChildCommand::with_read_only_root`] per read-only root,
    /// [`ChildCommand::with_read_only_file`] per file, [`ChildCommand::at_host_path`] per host
    /// path, [`ChildCommand::with_writable`] per writable workspace directory,
    /// [`ChildCommand::with_environment`] when the environment names a variable,
    /// [`ChildCommand::with_profile`], [`ChildCommand::with_network`] when `network` is not
    /// `None`, [`ChildCommand::with_egress`] per aperture (so an aperture makes the network
    /// [`NetworkMode::Aperture`] whatever `network` says), then, each only when the spec names
    /// it, [`ChildCommand::with_ca_bundle`], [`ChildCommand::with_timeout`],
    /// [`ChildCommand::with_stdin`] and [`ChildCommand::with_workdir`]. A spec naming nothing
    /// leaves the command as it was. Every check those builders' launch makes runs at the launch
    /// of the command returned, and no other: the limits (4 read-only roots, 16 files, 16 MiB of
    /// standard input, 4 egress apertures) and every refusal stay the builders'.
    ///
    /// Refused before anything starts, naming the field:
    ///
    /// - [`ConfinementRefusal::CapabilityUnserved`]: `writable_roots` not empty. Loom requests no
    ///   writable root outside the workspace, so a child never writes where the host did not
    ///   mean.
    /// - [`ConfinementRefusal::ScopeInvalid`]: a value the builder's own type cannot carry, never
    ///   narrowed: an aperture's `port` outside 0 to 65535, a `pinned` that is no socket address,
    ///   a negative `max_bytes` or `timeout_ms`, and one variable `environment.set` names twice.
    pub fn confined(self, spec: &ConfinementSpec) -> Result<Self, ConfinementError> {
        if !spec.writable_roots.is_empty() {
            return Err(refuse(
                ConfinementRefusal::CapabilityUnserved,
                format!(
                    "`writable_roots` names {} writable root(s); Loom requests no writable root outside the workspace",
                    spec.writable_roots.len()
                ),
            ));
        }
        let environment = environment(&spec.environment)?;
        let egress = spec
            .egress
            .iter()
            .enumerate()
            .map(|(index, aperture)| egress_aperture(index, aperture))
            .collect::<Result<Vec<_>, _>>()?;
        let timeout = spec
            .timeout_ms
            .map(|millis| {
                u64::try_from(millis)
                    .map(Duration::from_millis)
                    .map_err(|_| scope_error(format!("`timeout_ms` is {millis}, no duration")))
            })
            .transpose()?;

        let mut child = self;
        for root in &spec.read_only_roots {
            child = child.with_read_only_root(&root.host_path, &root.mount);
        }
        for file in &spec.read_only_files {
            child = child.with_read_only_file(&file.host_path, &file.mount);
        }
        for path in &spec.host_paths {
            child = child.at_host_path(path);
        }
        for path in &spec.writable {
            child = child.with_writable(path);
        }
        if !environment.allow.is_empty() || !environment.set.is_empty() {
            child = child.with_environment(environment);
        }
        child = child.with_profile(profile(spec.profile));
        if spec.network != ChildNetwork::None {
            child = child.with_network(network(spec.network));
        }
        for aperture in egress {
            child = child.with_egress(aperture);
        }
        if let Some(path) = &spec.ca_bundle {
            child = child.with_ca_bundle(path);
        }
        if let Some(timeout) = timeout {
            child = child.with_timeout(timeout);
        }
        if let Some(bytes) = &spec.stdin {
            child = child.with_stdin(bytes.clone());
        }
        if let Some(workdir) = &spec.workdir {
            child = child.with_workdir(workdir);
        }
        Ok(child)
    }
}

fn environment(environment: &ChildEnvironment) -> Result<ExecEnvironment, ConfinementError> {
    let mut set = BTreeMap::new();
    for entry in &environment.set {
        if set
            .insert(entry.name.clone(), entry.value.clone())
            .is_some()
        {
            return Err(scope_error(format!(
                "`environment.set` names the variable {:?} twice; a child's environment holds it once",
                entry.name
            )));
        }
    }
    Ok(ExecEnvironment {
        allow: environment.allow.iter().copied().map(variable).collect(),
        set,
    })
}

fn variable(variable: BaselineVariable) -> BaselineEnvironment {
    match variable {
        BaselineVariable::Lang => BaselineEnvironment::Lang,
        BaselineVariable::LcAll => BaselineEnvironment::LcAll,
        BaselineVariable::Path => BaselineEnvironment::Path,
        BaselineVariable::Term => BaselineEnvironment::Term,
        BaselineVariable::Tz => BaselineEnvironment::Tz,
    }
}

fn profile(profile: ChildProfile) -> SandboxProfile {
    match profile {
        ChildProfile::Workspace => SandboxProfile::Workspace,
    }
}

fn network(network: ChildNetwork) -> NetworkMode {
    match network {
        ChildNetwork::None => NetworkMode::None,
        ChildNetwork::Aperture => NetworkMode::Aperture,
    }
}

fn egress_aperture(
    index: usize,
    aperture: &ChildAperture,
) -> Result<EgressAperture, ConfinementError> {
    let port = u16::try_from(aperture.port).map_err(|_| {
        scope_error(format!(
            "`egress[{index}].port` is {}, outside 0 to 65535",
            aperture.port
        ))
    })?;
    let pinned: SocketAddr = aperture.pinned.parse().map_err(|_| {
        scope_error(format!(
            "`egress[{index}].pinned` is {:?}, no socket address",
            aperture.pinned
        ))
    })?;
    let max_bytes = aperture
        .max_bytes
        .map(|bytes| {
            u64::try_from(bytes).map_err(|_| {
                scope_error(format!(
                    "`egress[{index}].max_bytes` is {bytes}, below zero"
                ))
            })
        })
        .transpose()?;
    Ok(EgressAperture {
        name: aperture.name.clone(),
        host: aperture.host.clone(),
        port,
        pinned,
        max_bytes,
    })
}
