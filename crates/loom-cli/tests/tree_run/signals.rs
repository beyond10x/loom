//! A `tree run` stopped by SIGTERM or SIGINT while a bound program runs: the signal cancels the
//! run, which ends `Cancelled` with exit status 5, leaves no bound program running and leaves a
//! store every log of which verifies.
//!
//! The tree is one case of `tour-leg@1` whose program actions are bound to one program that writes
//! its pid and sleeps, so the first step is a long-running bound program; every program runs under
//! the `local` runner, and the test waits on the pid file, never a fixed time.

use std::fs;
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use super::{AT, Tree};

/// Both program actions of `tour-leg@1` bound to [`SLOW_PROGRAM`], so whichever the selector takes
/// first is a long-running bound program.
const SLOW_BINDINGS: &str = "format: loom.bindings/1
protocol: tour-leg@1
artifacts:
  route: route.txt
  notes: notes.txt
  ticket: ticket.txt
actions:
  - action: route.write
    executor:
      kind: program
      argv: [sh, stubs/slow.sh]
  - action: route.check
    executor:
      kind: program
      argv: [sh, stubs/slow.sh]
    evidence:
      - kind: route_check
        subject: route
        producer: route-check
        from: exit
        results: {\"0\": valid, \"*\": invalid}
";

const SLOW_PROGRAM: &str = "#!/bin/sh\necho $$ > slow.pid\nexec sleep 25\n";

/// Opens the slow one-case tree in `tree`'s store.
fn open_slow_tree(tree: &Tree) {
    fs::write(tree.workspace().join("stubs/slow.sh"), SLOW_PROGRAM).unwrap();
    fs::write(tree.workspace().join("slow.bindings.yaml"), SLOW_BINDINGS).unwrap();
    let opened = tree.open_leg_with("slow.bindings.yaml");
    assert_eq!(
        opened.status.code(),
        Some(0),
        "tree open: {}",
        String::from_utf8_lossy(&opened.stderr)
    );
}

/// `tree run --confinement local` on the store, started, not waited for.
fn start_run(tree: &Tree) -> Child {
    tree.command()
        .args([
            "tree",
            "run",
            "--confinement",
            "local",
            "--at",
            AT,
            "--store",
        ])
        .arg(tree.store())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// The pid the bound program wrote, waited for up to 15 s.
fn program_pid(tree: &Tree) -> i32 {
    let file = tree.workspace().join("slow.pid");
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(text) = fs::read_to_string(&file)
            && let Ok(pid) = text.trim().parse()
        {
            return pid;
        }
        assert!(Instant::now() < until, "the bound program never started");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Whether `pid` is a live process (present and not a zombie).
fn alive(pid: i32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat
            .rsplit_once(") ")
            .is_none_or(|(_, rest)| !rest.starts_with('Z')),
        Err(_) => false,
    }
}

/// Sends `signal` to `pid`.
fn signal(pid: i32, signal: i32) {
    // SAFETY: kill(2) with a pid this test started (or its bound program) and a valid signal.
    unsafe {
        libc::kill(pid, signal);
    }
}

/// Waits up to `bound` for `child` to end; kills it and panics past that.
fn wait_bounded(child: &mut Child, bound: Duration) -> ExitStatus {
    let until = Instant::now() + bound;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() > until {
            let _ = child.kill();
            panic!("b10x-loom did not end within {bound:?} of the signal");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Kills the bound program if it still runs, so a red case leaves nothing behind.
struct Reap(i32);

impl Drop for Reap {
    fn drop(&mut self) {
        if alive(self.0) {
            signal(self.0, libc::SIGKILL);
        }
    }
}

/// How a signalled `tree run` ended: its status, standard output and standard error.
struct Ended {
    program: Reap,
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

/// Starts the slow tree's run, waits for its bound program, sends `b10x-loom` `sent` and answers
/// how it ended.
fn signalled_run(tree: &Tree, sent: i32) -> Ended {
    open_slow_tree(tree);
    let mut run = start_run(tree);
    let program = Reap(program_pid(tree));
    signal(i32::try_from(run.id()).unwrap(), sent);
    let status = wait_bounded(&mut run, Duration::from_secs(10));
    let read = |stream: Option<&mut dyn std::io::Read>| {
        let mut text = String::new();
        if let Some(stream) = stream {
            let _ = stream.read_to_string(&mut text);
        }
        text
    };
    let stdout = read(run.stdout.as_mut().map(|s| s as &mut dyn std::io::Read));
    let stderr = read(run.stderr.as_mut().map(|s| s as &mut dyn std::io::Read));
    Ended {
        program,
        status,
        stdout,
        stderr,
    }
}

/// A `tree run` ended by SIGTERM (a supervisor's stop) while a bound program runs: `tree run
/// --help`'s exit 5, its document naming the stop `Cancelled`, not killed by the signal; the
/// program not running within 7 s of the run; and a store every log of which verifies. One test,
/// since each start of a bound program costs seconds of the binary's summed budget.
#[test]
fn a_terminated_tree_run_ends_cancelled_leaving_no_program_and_a_store_that_verifies() {
    let tree = Tree::new("sigterm");
    let ended = signalled_run(&tree, libc::SIGTERM);
    assert_eq!(
        ended.status.code(),
        Some(5),
        "b10x-loom tree run ended {} on SIGTERM: {}",
        ended.status,
        ended.stderr
    );
    let document: serde_json::Value = serde_json::from_str(ended.stdout.trim())
        .unwrap_or_else(|error| panic!("{error}: {}{}", ended.stdout, ended.stderr));
    assert_eq!(document["stop"], "Cancelled", "{document}");

    let until = Instant::now() + Duration::from_secs(7);
    while alive(ended.program.0) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !alive(ended.program.0),
        "the bound program {} still runs 7 s after b10x-loom tree run ended ({}) on SIGTERM: {}",
        ended.program.0,
        ended.status,
        ended.stderr
    );

    let verified = tree.tree("verify");
    assert_eq!(
        verified.status.code(),
        Some(0),
        "tree verify after a terminated run: {}{}",
        String::from_utf8_lossy(&verified.stdout),
        String::from_utf8_lossy(&verified.stderr)
    );
}

/// SIGINT (a terminal's Ctrl-C) cancels a `tree run` as SIGTERM does.
#[test]
fn an_interrupted_tree_run_ends_cancelled_and_leaves_no_bound_program_running() {
    let tree = Tree::new("sigint");
    let ended = signalled_run(&tree, libc::SIGINT);
    assert_eq!(
        ended.status.code(),
        Some(5),
        "b10x-loom tree run ended {} on SIGINT: {}",
        ended.status,
        ended.stderr
    );
    assert!(
        !alive(ended.program.0),
        "the bound program {} still runs after b10x-loom tree run ended on SIGINT",
        ended.program.0
    );
}
