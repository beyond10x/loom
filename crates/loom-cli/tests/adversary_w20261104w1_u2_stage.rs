//! Adversary pass 1 on `story:operator-constraints-in-ess` (wave 2026-11-04-w1, unit U2):
//! `b10x-loom agent stage-files` against `loom.resource.StageFiles` as
//! `ess/domains/resource.yaml` declares it. The declaration says the stage either refuses and
//! writes nothing, or stages every regular-file key "never through what stood there before" and
//! removes every other entry of `--to`.

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};

use b10x_loom_cli::stage::stage;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary_w20261104w1_u2_stage")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// A Secret volume as the kubelet lays it out: `..data` linked to a timestamped directory, each
/// key linked to `..data/<key>`.
fn secret_volume(dir: &Path, keys: &[(&str, &str)]) {
    fs::create_dir_all(dir).expect("from");
    let stamped = dir.join("..2026_11_04_00_00_00.000000000");
    fs::create_dir(&stamped).expect("stamped dir");
    for (key, value) in keys {
        fs::write(stamped.join(key), value).expect("key");
    }
    symlink("..2026_11_04_00_00_00.000000000", dir.join("..data")).expect("data link");
    for (key, _) in keys {
        symlink(format!("..data/{key}"), dir.join(key)).expect("key link");
    }
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path)
        .expect("mode")
        .permissions()
        .mode()
        & 0o777
}

/// `--to` holds a directory that a regular-file key names. The facts the command reads give
/// `stale_directory: false` (a key names it), so `StageFiles` answers `staged`; the command then
/// fails at `remove_file` on that directory, after it has already set `--to`'s mode and copied
/// the keys before it in name order. The declaration allows neither half: a staged answer stages
/// every key, and a stage that does not stage writes nothing.
#[test]
fn adv_u2_a_directory_a_key_names_is_staged_or_refused_before_any_write() {
    let dir = scratch("key-named-directory");
    let from = dir.join("secret");
    secret_volume(&from, &[("a-token", "fresh-a"), ("b-token", "fresh-b")]);
    let to = dir.join("staged");
    fs::create_dir(&to).expect("to");
    fs::set_permissions(&to, fs::Permissions::from_mode(0o755)).expect("mode");
    fs::write(to.join("dropped"), "old").expect("stale file");
    fs::create_dir(to.join("b-token")).expect("a directory a key names");

    match stage(&from, &to) {
        Ok(staged) => {
            assert_eq!(staged, 2);
            assert_eq!(
                fs::read_to_string(to.join("b-token")).expect("b-token staged"),
                "fresh-b"
            );
        }
        Err(why) => {
            let wrote = [
                (mode(&to) != 0o755).then(|| format!("--to mode set to {:o}", mode(&to))),
                to.join("a-token")
                    .exists()
                    .then(|| "a-token copied".to_owned()),
                (!to.join("dropped").exists()).then(|| "dropped removed".to_owned()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            assert!(
                wrote.is_empty(),
                "the stage failed ({why}) after writing: {}",
                wrote.join(", ")
            );
        }
    }
}

/// `--to` given with a trailing slash names a link to a directory. `lstat("link/")` resolves the
/// link, so the command reads `Directory` where `StageDestination` declares `Link`, and stages
/// through the link into the directory it points at.
#[test]
fn adv_u2_a_link_named_with_a_trailing_slash_is_refused_as_a_link() {
    let dir = scratch("trailing-slash-link");
    let from = dir.join("secret");
    secret_volume(&from, &[("token", "value")]);
    let real = dir.join("real");
    fs::create_dir(&real).expect("real");
    let link = dir.join("link");
    symlink(&real, &link).expect("link");
    let mut to = link.into_os_string();
    to.push("/");
    let to = PathBuf::from(to);

    let answer = stage(&from, &to);

    let through: Vec<String> = fs::read_dir(&real)
        .expect("read real")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        answer.is_err() && through.is_empty(),
        "answer {answer:?}; written through the link: {through:?}"
    );
}
