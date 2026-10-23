//! A child started in the working directory the host names (`story:child-workdir`): each refusal
//! a working directory gets before anything starts, the driver's `exec.workdir` fact, what the
//! request asks Substrate for, and the record a launch accepts.

use super::*;

/// The refusal `check_workdir` gives `workdir` for a child in `/work/space` reading the root
/// `/srv/tools` at `/opt/tools` and the file `/srv/policy.toml` at `/etc/loom/policy.toml`, read
/// as text only: every case here is refused before the disk is asked.
fn lexical_refusal(workdir: impl Into<PathBuf>) -> ConfinementError {
    let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .with_read_only_root("/srv/tools", "/opt/tools")
        .with_read_only_file("/srv/policy.toml", "/etc/loom/policy.toml")
        .with_workdir(workdir);
    check_workdir(&child).expect_err("refused")
}

fn assert_refused(workdir: &str, why: &str) {
    let error = lexical_refusal(workdir);
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains(workdir), "{workdir}: {error}");
    assert!(error.detail.contains(why), "{workdir} ({why}): {error}");
}

#[test]
fn a_workdir_that_is_not_absolute_and_normal_is_refused_naming_it() {
    for workdir in ["workspace", "sub", ""] {
        assert_refused(workdir, "not absolute");
    }
    for workdir in [
        "/workspace/../etc",
        "/workspace/./sub",
        "/workspace/..",
        "/workspace//sub",
        "/workspace/sub/",
        "/workspace/",
    ] {
        assert_refused(workdir, "`.`, `..` or empty component");
    }
}

/// A working directory holding a control character (C0, DEL or C1, a newline and a NUL
/// included) is refused before the disk is asked, the refusal naming it escaped, so the refusal
/// is one line holding no control character.
#[test]
fn a_workdir_holding_a_control_character_is_refused_escaped() {
    for workdir in [
        "/workspace/a\nb",
        "/workspace/a\rb",
        "/workspace/a\0b",
        "/workspace/\u{1b}[2J",
        "/workspace/a\u{7f}",
        "/workspace/a\u{85}b",
        "/workspace/a\tb",
        "relative\n",
    ] {
        let error = lexical_refusal(workdir);
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
        assert!(error.detail.contains("control character"), "{error}");
        assert!(
            error.detail.contains(&format!("{workdir:?}")),
            "{workdir:?}: {error}"
        );
        assert!(
            !error.detail.chars().any(char::is_control),
            "{workdir:?}: {error}"
        );
    }
}

/// A directory this process cannot search, or one below such a directory, is refused before
/// launch, naming the working directory, in the workspace and under a read-only root alike; a
/// directory with search permission alone is entered.
#[test]
fn a_workdir_the_process_cannot_search_is_refused_naming_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let held = tempfile::tempdir().unwrap();
    let base = held.path().canonicalize().unwrap();
    let workspace = base.join("workspace");
    let tools = base.join("tools");
    for directory in [
        workspace.join("locked"),
        workspace.join("outer/inner"),
        workspace.join("search-only"),
        tools.join("locked"),
    ] {
        std::fs::create_dir_all(directory).unwrap();
    }
    let mode = |path: &Path, mode: u32| {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    mode(&workspace.join("locked"), 0o600);
    mode(&workspace.join("outer"), 0o600);
    mode(&workspace.join("search-only"), 0o100);
    mode(&tools.join("locked"), 0o600);
    let child = |workdir: &str| {
        ChildCommand::new("true", [] as [&str; 0], &workspace)
            .with_read_only_root(&tools, "/opt/tools")
            .with_workdir(workdir)
    };
    let searchable = may_search(&workspace.join("locked"));
    for workdir in [
        "/workspace/locked",
        "/workspace/outer",
        "/workspace/outer/inner",
        "/opt/tools/locked",
    ] {
        let outcome = check_workdir(&child(workdir));
        if searchable {
            // A process that may search any directory (`CAP_DAC_OVERRIDE`) enters these too.
            outcome.unwrap();
            continue;
        }
        let error = outcome.expect_err("refused");
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
        assert!(error.detail.contains(workdir), "{workdir}: {error}");
        assert!(
            error.detail.contains("not searchable"),
            "{workdir}: {error}"
        );
    }
    check_workdir(&child("/workspace/search-only")).unwrap();
    check_workdir(&child("/opt/tools")).unwrap();
    mode(&workspace.join("outer"), 0o700);
}

/// An ended run, of `input`, that wrote `stdout` and `stderr` and exited 1.
fn ended(input: &ExecStartInput, stdout: &str, stderr: &str) -> Ending {
    Ending {
        record: Some(record(input, input.workdir.as_deref())),
        state: ExecState::Exited,
        exit: Some(ExecExit {
            code: Some(1),
            signal: None,
        }),
        refusal: None,
        output: ChildOutput {
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            stdout_truncated: false,
            stderr_truncated: false,
        },
        output_complete: true,
    }
}

/// A run naming a working directory whose output carries bubblewrap's `Can't chdir to` diagnostic
/// for it is refused once it ended, `run_child` and a live child's `wait` alike (both settle the
/// run here), so a program that never ran reports no exit status; the same text from a run
/// naming no working directory, or naming another directory, is the child's own output.
#[test]
fn a_run_bubblewrap_could_not_enter_its_workdir_is_refused() {
    let named = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .with_workdir("/workspace/locked")
        .input("space", "snapshot".to_owned());
    let diagnostic = "bwrap: Can't chdir to /workspace/locked: Permission denied\n";
    for (stdout, stderr) in [
        ("", diagnostic),
        (diagnostic, ""),
        ("", &*format!("noise\n{diagnostic}")),
    ] {
        let error = settle(
            &named,
            &[],
            "digest".to_owned(),
            ended(&named, stdout, stderr),
        )
        .err()
        .expect("refused");
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
        assert!(error.detail.contains("/workspace/locked"), "{error}");
        assert!(error.detail.contains("never ran"), "{error}");
    }
    let launched = settle(
        &named,
        &[],
        "digest".to_owned(),
        ended(
            &named,
            "",
            "bwrap: Can't chdir to /workspace/locked2: Permission denied\n",
        ),
    )
    .expect("another directory's diagnostic is the child's output");
    assert_eq!(launched.execution.exit_code, Some(1));
    let plain = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .input("space", "snapshot".to_owned());
    let launched = settle(
        &plain,
        &[],
        "digest".to_owned(),
        ended(&plain, "", diagnostic),
    )
    .expect("a run naming no working directory is not judged by it");
    assert_eq!(launched.execution.exit_code, Some(1));
}

#[test]
fn a_workdir_that_is_not_utf8_is_refused_naming_it() {
    use std::os::unix::ffi::OsStrExt as _;
    let error = lexical_refusal(std::ffi::OsStr::from_bytes(b"/workspace/\xff"));
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains("/workspace/"), "{error}");
    assert!(error.detail.contains("not UTF-8"), "{error}");
}

#[test]
fn a_workdir_longer_than_substrate_admits_is_refused_naming_it() {
    let long = format!("/workspace/{}", "a".repeat(MAX_WORKDIR_BYTES));
    let error = lexical_refusal(long.as_str());
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains("/workspace/aaaa"), "{error}");
    assert!(
        error.detail.contains(&format!("{} bytes", long.len())),
        "{error}"
    );
    assert!(error.detail.contains("4096"), "{error}");
    // The bound itself passes the text check and reaches the disk, where the workspace is absent.
    let bound = format!("/workspace/{}", "a".repeat(MAX_WORKDIR_BYTES - 11));
    assert_eq!(bound.len(), MAX_WORKDIR_BYTES);
    let error = lexical_refusal(bound.as_str());
    assert!(!error.detail.contains("4096"), "{error}");
}

#[test]
fn a_workdir_outside_the_workspace_and_every_root_is_refused_naming_it() {
    for workdir in [
        "/",
        "/scratch",
        "/scratch/x",
        "/runtime",
        "/runtime/x",
        "/tmp",
        "/usr/bin",
        "/workspacex",
        "/opt",
        "/opt/toolsx",
        "/srv/tools",
    ] {
        assert_refused(workdir, "not at or beneath /workspace or a read-only root");
    }
}

#[test]
fn a_workdir_at_or_beneath_a_read_only_file_is_refused_naming_it() {
    for workdir in ["/etc/loom/policy.toml", "/etc/loom/policy.toml/x"] {
        assert_refused(workdir, "read-only file");
    }
}

#[test]
fn a_child_without_a_workdir_passes_and_asks_for_none() {
    let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
    assert_eq!(child.workdir(), None);
    check_workdir(&child).unwrap();
    let input = child.input("space", "snapshot".to_owned());
    assert_eq!(input.workdir, None);
    let wire = serde_json::to_value(&input).unwrap();
    assert!(wire.get("workdir").is_none(), "{wire}");
    let session =
        serde_json::to_value(child.session_input("space", "snapshot".to_owned())).unwrap();
    assert!(session["exec"].get("workdir").is_none(), "{session}");
}

#[test]
fn a_named_workdir_is_asked_for_as_named_in_both_sessions() {
    let child =
        ChildCommand::new("true", [] as [&str; 0], "/work/space").with_workdir("/workspace/sub");
    assert_eq!(child.workdir(), Some(Path::new("/workspace/sub")));
    let input = child.input("space", "snapshot".to_owned());
    assert_eq!(input.workdir.as_deref(), Some("/workspace/sub"));
    let session = child.session_input("space", "snapshot".to_owned());
    assert_eq!(session.exec.workdir.as_deref(), Some("/workspace/sub"));
}

#[test]
fn a_driver_without_the_workdir_fact_refuses_a_workdir_unserved() {
    let named =
        ChildCommand::new("true", [] as [&str; 0], "/work/space").with_workdir("/workspace/sub");
    let plain = ChildCommand::new("true", [] as [&str; 0], "/work/space");
    for published in [None, Some(false)] {
        let facts = CapabilityFacts {
            exec_workdir: published,
            ..CapabilityFacts::default()
        };
        let error = check_workdir_served(&named, &facts).expect_err("unserved");
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{error}"
        );
        assert!(error.detail.contains("exec.workdir"), "{error}");
        assert!(error.detail.contains("/workspace/sub"), "{error}");
        check_workdir_served(&plain, &facts).unwrap();
    }
    let served = CapabilityFacts {
        exec_workdir: Some(true),
        ..CapabilityFacts::default()
    };
    check_workdir_served(&named, &served).unwrap();
    check_workdir_served(&plain, &served).unwrap();
}

/// Substrate's record of a launch of `input` that applied exactly what it asked for, with the
/// working directory `workdir`.
fn record(input: &ExecStartInput, workdir: Option<&str>) -> substrate_wire::AppliedConfinement {
    substrate_wire::AppliedConfinement {
        capability_snapshot: input.sandbox.capability_snapshot.clone(),
        cgroup: "substrate-ex_loom_1_0".to_owned(),
        filesystem: substrate_wire::AppliedFilesystem::WorkspaceReadWriteSystemReadOnly,
        network: AppliedNetwork::None,
        profile: input.sandbox.profile,
        workspace_access: input.workspace_access.clone(),
        capsule: None,
        read_only_roots: input.read_only_roots.clone(),
        read_only_files: input.read_only_files.clone(),
        secret_slots: Vec::new(),
        workdir: workdir.map(str::to_owned),
        writable_roots: Vec::new(),
    }
}

#[test]
fn a_record_naming_another_workdir_is_refused() {
    let named = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .with_workdir("/workspace/sub")
        .input("space", "snapshot".to_owned());
    check_record(&named, &[], &record(&named, Some("/workspace/sub"))).unwrap();
    for other in [None, Some("/workspace"), Some("/workspace/other")] {
        let error = check_record(&named, &[], &record(&named, other)).expect_err("refused");
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{other:?}"
        );
    }
    let plain = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .input("space", "snapshot".to_owned());
    check_record(&plain, &[], &record(&plain, None)).unwrap();
    for other in [Some("/workspace"), Some("/workspace/sub")] {
        let error = check_record(&plain, &[], &record(&plain, other)).expect_err("refused");
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{other:?}"
        );
    }
}

#[test]
fn a_workdir_changes_the_digest_and_its_absence_does_not() {
    let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
    let input = child.input("space", "snapshot".to_owned());
    let profile = requested_profile(&input, &[]);
    let plain = profile_digest(&profile, &[], &[], None, &[], None);
    let named = profile_digest(&profile, &[], &[], None, &[], Some("/workspace/sub"));
    assert_ne!(plain, named);
    assert_ne!(
        named,
        profile_digest(&profile, &[], &[], None, &[], Some("/workspace/other"))
    );
    // The digest a launch without a working directory computed before it existed.
    let before = serde_json::json!({ "backend":"substrate", "network":profile.network,
        "writable_scopes":profile.writable_scopes, "read_only_roots":[], "environment":[],
        "timeout_ms":profile.timeout_ms, "memory_bytes":profile.memory_bytes,
        "processes":profile.processes, "cpu_millis":profile.cpu_millis });
    assert_eq!(
        plain,
        format!("{:x}", Sha256::digest(before.to_string().as_bytes()))
    );
}

/// The applied confinement a launch reports names the working directory Substrate recorded, and
/// none without one.
#[test]
fn the_applied_confinement_names_the_recorded_workdir() {
    for workdir in [None, Some("/workspace/sub")] {
        let child = workdir.into_iter().fold(
            ChildCommand::new("true", [] as [&str; 0], "/work/space"),
            ChildCommand::with_workdir,
        );
        let input = child.input("space", "snapshot".to_owned());
        let ending = Ending {
            record: Some(record(&input, workdir)),
            state: ExecState::Exited,
            exit: Some(ExecExit {
                code: Some(0),
                signal: None,
            }),
            refusal: None,
            output: ChildOutput {
                stdout: Vec::new(),
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
            output_complete: true,
        };
        let launched = settle(&input, &[], "digest".to_owned(), ending).expect("settled");
        assert_eq!(launched.execution.applied.workdir.as_deref(), workdir);
    }
}
