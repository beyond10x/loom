//! The forge's dependency cache (`story:forge-build-cache`), held over every workflow under
//! `.github/workflows/`: only `deps-cache.yml`, on a push to `main`, writes the entry;
//! `loom / task check` restores it under the same key, and no workflow saves elsewhere, uses an
//! unpinned cache action or a compile cache wrapper (`~/beyond10x/AGENTS.md` § One compile cache).

#[cfg(test)]
mod tests {
    use serde_yaml_ng::Value;

    /// The runner image both jobs pin and the key names.
    const IMAGE: &str = "ubuntu-24.04";
    /// The rustc hash of the `dtolnay/rust-toolchain` step `id: rust`.
    const RUSTC_HASH: &str = "steps.rust.outputs.cachekey";
    /// The key of After 3, in `loom / task check`'s restore step and in `deps-cache.yml`.
    const KEY: &str = "loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('rust-toolchain.toml', 'Cargo.lock', '.cargo/config.toml', '**/Cargo.toml') }}";
    /// The files the key hashes.
    const HASHED: [&str; 4] = [
        "'rust-toolchain.toml'",
        "'Cargo.lock'",
        "'.cargo/config.toml'",
        "'**/Cargo.toml'",
    ];
    /// What the entry holds.
    const PATHS: [&str; 4] = [
        "~/.cargo/registry",
        "~/.cargo/git",
        "target",
        "consumers/pinned/target",
    ];
    /// A compile cache wrapper on the forge (`~/beyond10x/AGENTS.md` § One compile cache).
    const WRAPPERS: [&str; 3] = ["RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "SCCACHE_"];
    /// The restore step of job `check`.
    const RESTORE: &str = "Restore the dependency build";
    /// The workflow that writes the entry.
    const WRITER: &str = "deps-cache.yml";

    #[derive(Clone, Copy, PartialEq)]
    enum Cache {
        Combined,
        Restore,
        Save,
    }

    /// Every way `workflows` (file name, parsed workflow) breaks the forge's dependency cache,
    /// each naming the file and the job or step.
    fn problems(workflows: &[(String, Value)]) -> Vec<String> {
        let mut found = Vec::new();
        let mut keys: Vec<(String, String)> = Vec::new();
        for (file, workflow) in workflows {
            wrappers(
                &format!("{file}: the workflow"),
                workflow,
                Some("jobs"),
                &mut found,
            );
            let saves_allowed = pushes_to_main_only(&workflow["on"]);
            for (job_name, job) in jobs(workflow) {
                wrappers(
                    &format!("{file}: job {job_name}"),
                    job,
                    Some("steps"),
                    &mut found,
                );
                let steps = steps(job);
                if serde_yaml_ng::to_string(job)
                    .unwrap_or_default()
                    .contains("steps.rust.outputs")
                    && !steps.iter().any(|step| {
                        uses(step).starts_with("dtolnay/rust-toolchain@")
                            && step["id"].as_str() == Some("rust")
                    })
                {
                    found.push(format!(
                        "{file}: job {job_name} reads steps.rust.outputs, and its \
                         dtolnay/rust-toolchain step has no `id: rust`"
                    ));
                }
                for (index, step) in steps.iter().enumerate() {
                    let at = format!("{file}: job {job_name}, step {}", label(step, index));
                    wrappers(&at, step, None, &mut found);
                    let action = uses(step);
                    let Some(kind) = cache_kind(action) else {
                        continue;
                    };
                    if !pinned(action) {
                        found.push(format!(
                            "{at}: {action} is not pinned by a 40-hex commit digest"
                        ));
                    }
                    match kind {
                        Cache::Combined => found.push(format!(
                            "{at}: {action} saves the entry in its post step; restore with \
                             actions/cache/restore and save with actions/cache/save"
                        )),
                        Cache::Save => {
                            if !saves_allowed {
                                found.push(format!(
                                    "{at}: saves the cache outside a push to main (`on:` is not \
                                     `push: branches: [main]` alone)"
                                ));
                            }
                            save_key(&at, step, steps, &mut found);
                            paths(&at, step, &mut found);
                        }
                        Cache::Restore => {
                            restore_key(&at, job, step, &mut found);
                            paths(&at, step, &mut found);
                            keys.push((
                                at.clone(),
                                step["with"]["key"].as_str().unwrap_or_default().to_owned(),
                            ));
                        }
                    }
                }
            }
        }
        check_job(workflows, &mut found);
        writer(workflows, &mut found);
        let mut distinct: Vec<&str> = keys.iter().map(|(_, key)| key.as_str()).collect();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() > 1 {
            let named: Vec<String> = keys
                .iter()
                .map(|(at, key)| format!("{at}: {key}"))
                .collect();
            found.push(format!("the keys differ:\n  {}", named.join("\n  ")));
        }
        found
    }

    fn jobs(workflow: &Value) -> Vec<(&str, &Value)> {
        workflow["jobs"]
            .as_mapping()
            .into_iter()
            .flatten()
            .map(|(name, job)| (name.as_str().unwrap_or_default(), job))
            .collect()
    }

    fn steps(job: &Value) -> &[Value] {
        job["steps"].as_sequence().map(Vec::as_slice).unwrap_or(&[])
    }

    fn uses(step: &Value) -> &str {
        step["uses"].as_str().unwrap_or_default()
    }

    fn label(step: &Value, index: usize) -> String {
        step["name"]
            .as_str()
            .or_else(|| step["uses"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("#{index}"))
    }

    /// Which cache action `action` is; the owner and name compare without case, as GitHub's do.
    fn cache_kind(action: &str) -> Option<Cache> {
        let name = action
            .split('@')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match name.trim_end_matches('/') {
            "actions/cache" => Some(Cache::Combined),
            "actions/cache/restore" => Some(Cache::Restore),
            "actions/cache/save" => Some(Cache::Save),
            _ => None,
        }
    }

    fn pinned(action: &str) -> bool {
        action
            .rsplit_once('@')
            .is_some_and(|(_, pin)| pin.len() == 40 && pin.bytes().all(|b| b.is_ascii_hexdigit()))
    }

    /// `on:` is `push: branches: [main]` and nothing else.
    fn pushes_to_main_only(on: &Value) -> bool {
        let push: Value = serde_yaml_ng::from_str("branches: [main]").unwrap();
        on.as_mapping()
            .is_some_and(|on| on.len() == 1 && on.get("push") == Some(&push))
    }

    /// Every key or text under `value` (but the top-level key `skip`) that names a wrapper.
    fn wrappers(at: &str, value: &Value, skip: Option<&str>, found: &mut Vec<String>) {
        fn walk(value: &Value, texts: &mut Vec<String>) {
            match value {
                Value::String(text) => texts.push(text.clone()),
                Value::Sequence(items) => items.iter().for_each(|item| walk(item, texts)),
                Value::Mapping(map) => {
                    for (key, item) in map {
                        walk(key, texts);
                        walk(item, texts);
                    }
                }
                Value::Tagged(tagged) => walk(&tagged.value, texts),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        match (value.as_mapping(), skip) {
            (Some(map), Some(skip)) => {
                for (key, item) in map {
                    if key.as_str() != Some(skip) {
                        walk(key, &mut texts);
                        walk(item, &mut texts);
                    }
                }
            }
            _ => walk(value, &mut texts),
        }
        for text in texts {
            for word in WRAPPERS {
                if text.contains(word) {
                    found.push(format!(
                        "{at}: `{text}` names {word}; the forge builds without a compile cache \
                         wrapper (~/beyond10x/AGENTS.md § One compile cache)"
                    ));
                }
            }
        }
    }

    fn paths(at: &str, step: &Value, found: &mut Vec<String>) {
        let got: Vec<&str> = step["with"]["path"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        if got != PATHS {
            found.push(format!("{at}: its paths are {got:?}, not {PATHS:?}"));
        }
    }

    /// The key holds the job's pinned image, the rustc hash and the hashed files; `restore-keys`
    /// (where the step restores) holds the image and the rustc hash.
    fn restore_key(at: &str, job: &Value, step: &Value, found: &mut Vec<String>) {
        let image = job["runs-on"].as_str().unwrap_or_default();
        let key = step["with"]["key"].as_str().unwrap_or_default();
        if image != IMAGE || !key.contains(image) {
            found.push(format!(
                "{at}: the job runs on `{image}` and the key `{key}` must name the pinned image \
                 {IMAGE}"
            ));
        }
        if !key.contains(RUSTC_HASH) {
            found.push(format!(
                "{at}: the key `{key}` does not hold the rustc hash {RUSTC_HASH}"
            ));
        }
        for file in HASHED {
            if !key.contains("hashFiles(") || !key.contains(file) {
                found.push(format!("{at}: the key `{key}` does not hash {file}"));
            }
        }
        let lookup = step["with"]["lookup-only"].as_bool() == Some(true)
            || step["with"]["lookup-only"].as_str() == Some("true");
        if !lookup {
            let prefix = step["with"]["restore-keys"].as_str().unwrap_or_default();
            if !prefix
                .lines()
                .any(|line| line.contains(IMAGE) && line.contains(RUSTC_HASH))
            {
                found.push(format!(
                    "{at}: its restore-keys `{}` do not hold the image name {IMAGE} and the rustc \
                     hash {RUSTC_HASH}",
                    prefix.trim()
                ));
            }
        }
    }

    /// A save step saves under the key a restore step of its job resolved.
    fn save_key(at: &str, step: &Value, steps: &[Value], found: &mut Vec<String>) {
        let key = step["with"]["key"].as_str().unwrap_or_default();
        let resolved = key
            .trim()
            .strip_prefix("${{")
            .and_then(|key| key.strip_suffix("}}"))
            .map(str::trim)
            .and_then(|key| key.strip_prefix("steps."))
            .and_then(|key| key.strip_suffix(".outputs.cache-primary-key"));
        let restores = resolved.is_some_and(|id| {
            steps.iter().any(|other| {
                other["id"].as_str() == Some(id)
                    && cache_kind(uses(other)) == Some(Cache::Restore)
                    && other["with"]["lookup-only"].as_bool() != Some(true)
            })
        });
        if !restores {
            found.push(format!(
                "{at}: saves under `{key}`, not the key a restore step of the job resolved \
                 (`${{{{ steps.<id>.outputs.cache-primary-key }}}}`)"
            ));
        }
    }

    /// Job `check` of `check.yml` restores under [`KEY`] after `id: rust` and before
    /// `cargo fetch --locked`, and its restore and `lscpu` steps skip with the run's class.
    fn check_job(workflows: &[(String, Value)], found: &mut Vec<String>) {
        let Some((_, workflow)) = workflows.iter().find(|(file, _)| file == "check.yml") else {
            found.push("no check.yml under .github/workflows".to_owned());
            return;
        };
        let steps = steps(&workflow["jobs"]["check"]);
        let rust = steps
            .iter()
            .position(|step| step["id"].as_str() == Some("rust"));
        let fetch = steps.iter().position(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains("cargo fetch --locked"))
        });
        let restore = steps
            .iter()
            .position(|step| step["name"].as_str() == Some(RESTORE));
        let at = format!("check.yml: job check, step {RESTORE}");
        match restore {
            None => found.push(format!("check.yml: job check has no step {RESTORE}")),
            Some(index) => {
                let step = &steps[index];
                if cache_kind(uses(step)) != Some(Cache::Restore) {
                    found.push(format!(
                        "{at}: uses `{}`, not actions/cache/restore",
                        uses(step)
                    ));
                }
                if step["with"]["key"].as_str() != Some(KEY) {
                    found.push(format!(
                        "{at}: its key `{}` is not the key `{KEY}`",
                        step["with"]["key"].as_str().unwrap_or_default()
                    ));
                }
                if !rust.is_some_and(|rust| rust < index) {
                    found.push(format!("{at}: it must sit after the step `id: rust`"));
                }
                if !fetch.is_some_and(|fetch| index < fetch) {
                    found.push(format!("{at}: it must sit before `cargo fetch --locked`"));
                }
                skips_with_the_class(&at, step, found);
            }
        }
        match steps.iter().position(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains("lscpu"))
        }) {
            None => found.push("check.yml: job check has no step that runs `lscpu`".to_owned()),
            Some(index) => skips_with_the_class(
                &format!("check.yml: job check, step {}", label(&steps[index], index)),
                &steps[index],
                found,
            ),
        }
    }

    fn skips_with_the_class(at: &str, step: &Value, found: &mut Vec<String>) {
        if !step["if"]
            .as_str()
            .is_some_and(|condition| condition.contains("steps.changes.outputs.class"))
        {
            found.push(format!(
                "{at}: its `if:` does not name steps.changes.outputs.class"
            ));
        }
    }

    /// `deps-cache.yml` exists, reads contents only, its jobs end at 40 minutes, and one
    /// concurrency group of the workflow never cancels a run in progress.
    fn writer(workflows: &[(String, Value)], found: &mut Vec<String>) {
        let Some((_, workflow)) = workflows.iter().find(|(file, _)| file == WRITER) else {
            found.push(format!(
                "no {WRITER} under .github/workflows: nothing writes the dependency cache"
            ));
            return;
        };
        let read: Value = serde_yaml_ng::from_str("contents: read").unwrap();
        if workflow["permissions"] != read {
            found.push(format!(
                "{WRITER}: the workflow lacks `permissions: contents: read` (and nothing else)"
            ));
        }
        for (name, job) in jobs(workflow) {
            if !job["permissions"].is_null() && job["permissions"] != read {
                found.push(format!(
                    "{WRITER}: job {name} holds permissions beyond `contents: read`"
                ));
            }
            if job["timeout-minutes"].as_u64() != Some(40) {
                found.push(format!("{WRITER}: job {name} has no `timeout-minutes: 40`"));
            }
        }
        let concurrency = &workflow["concurrency"];
        match concurrency["group"].as_str() {
            None => found.push(format!(
                "{WRITER}: the workflow has no workflow-level `concurrency:` group"
            )),
            Some(group) if group.contains("${{") => found.push(format!(
                "{WRITER}: the workflow's concurrency group `{group}` must be one per workflow, \
                 not per ref"
            )),
            Some(_) => {}
        }
        if concurrency["cancel-in-progress"].as_bool() != Some(false) {
            found.push(format!(
                "{WRITER}: the workflow's concurrency lacks `cancel-in-progress: false`"
            ));
        }
    }

    const CHECK: &str = r#"
name: check
on:
  pull_request:
  push:
    branches: [main]
  workflow_call:
permissions:
  contents: read
jobs:
  check:
    name: loom / task check
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c
        id: rust
        with:
          toolchain: 1.99.1
      - name: Print the runner's CPU model
        if: steps.changes.outputs.class != 'skip'
        run: lscpu | grep 'Model name'
      - name: Restore the dependency build
        if: steps.changes.outputs.class != 'skip'
        uses: actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            consumers/pinned/target
          key: loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('rust-toolchain.toml', 'Cargo.lock', '.cargo/config.toml', '**/Cargo.toml') }}
          restore-keys: |
            loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-
      - name: Fetch locked dependencies for offline checks
        run: cargo fetch --locked
      - name: task check
        run: task check
  macos:
    name: loom / macos build
    runs-on: macos-15
    steps:
      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c
        with:
          toolchain: 1.99.1
      - run: cargo fetch --locked
"#;

    const DEPS: &str = r#"
name: deps-cache
on:
  push:
    branches: [main]
concurrency:
  group: deps-cache
  cancel-in-progress: false
permissions:
  contents: read
jobs:
  deps:
    name: loom / dependency cache
    runs-on: ubuntu-24.04
    timeout-minutes: 40
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c
        id: rust
        with:
          toolchain: 1.99.1
      - name: Look the dependency build up
        id: lookup
        uses: actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            consumers/pinned/target
          key: loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('rust-toolchain.toml', 'Cargo.lock', '.cargo/config.toml', '**/Cargo.toml') }}
          lookup-only: true
      - name: Restore an earlier dependency build
        id: restore
        if: steps.lookup.outputs.cache-hit != 'true'
        uses: actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            consumers/pinned/target
          key: loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('rust-toolchain.toml', 'Cargo.lock', '.cargo/config.toml', '**/Cargo.toml') }}
          restore-keys: |
            loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-
      - name: Build the test targets
        if: steps.lookup.outputs.cache-hit != 'true'
        run: cargo test --workspace --locked --no-run
      - name: Save the dependency build
        if: steps.lookup.outputs.cache-hit != 'true'
        uses: actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            consumers/pinned/target
          key: ${{ steps.restore.outputs.cache-primary-key }}
"#;

    const RESTORE_AT: &str =
        "        uses: actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9";
    const KEY_LINE: &str = "          key: loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('rust-toolchain.toml', 'Cargo.lock', '.cargo/config.toml', '**/Cargo.toml') }}";

    fn parsed(check: &str, deps: &str) -> Vec<(String, Value)> {
        vec![
            (
                "check.yml".to_owned(),
                serde_yaml_ng::from_str(check).unwrap(),
            ),
            (
                "deps-cache.yml".to_owned(),
                serde_yaml_ng::from_str(deps).unwrap(),
            ),
        ]
    }

    /// `text` with its one occurrence of `from` replaced, so a planted defect that no longer
    /// applies fails the case instead of passing it.
    fn swap(text: &str, from: &str, to: &str) -> String {
        assert_eq!(
            text.matches(from).count(),
            1,
            "`{from}` is not in the input once"
        );
        text.replacen(from, to, 1)
    }

    /// The two inputs pass, and with the defect planted the guard names `expected`.
    fn fails(check: &str, deps: &str, expected: &[&str]) {
        assert_eq!(problems(&parsed(CHECK, DEPS)), Vec::<String>::new());
        let found = problems(&parsed(check, deps));
        for part in expected {
            assert!(
                found.iter().any(|problem| problem.contains(part)),
                "no problem names `{part}`: {found:#?}"
            );
        }
    }

    #[test]
    fn a_save_outside_a_push_to_main_fails() {
        let save = "      - name: task check\n        run: task check\n      - name: Save\n        uses: actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9\n        with:\n          path: target\n          key: x\n";
        fails(
            &swap(
                CHECK,
                "      - name: task check\n        run: task check\n",
                save,
            ),
            DEPS,
            &["check.yml: job check, step Save", "outside a push to main"],
        );
        fails(
            CHECK,
            &swap(DEPS, "on:\n  push:\n", "on:\n  pull_request:\n  push:\n"),
            &[
                "deps-cache.yml: job deps, step Save the dependency build",
                "outside a push to main",
            ],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "    branches: [main]\n",
                "    branches: [main]\n    tags: ['*']\n",
            ),
            &[
                "deps-cache.yml: job deps, step Save the dependency build",
                "outside a push to main",
            ],
        );
    }

    #[test]
    fn a_combined_cache_action_fails_anywhere() {
        fails(
            &swap(
                CHECK,
                RESTORE_AT,
                "        uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9",
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "post step",
            ],
        );
    }

    #[test]
    fn an_unpinned_cache_action_fails() {
        fails(
            &swap(CHECK, RESTORE_AT, "        uses: actions/cache/restore@v6"),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "40-hex",
            ],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "        uses: actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9",
                "        uses: actions/cache/save@55cc834",
            ),
            &[
                "deps-cache.yml: job deps, step Save the dependency build",
                "40-hex",
            ],
        );
    }

    #[test]
    fn a_compile_cache_wrapper_fails() {
        for word in ["RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "SCCACHE_DIR"] {
            fails(
                &swap(
                    CHECK,
                    "        run: task check\n",
                    &format!("        run: task check\n        env:\n          {word}: sccache\n"),
                ),
                DEPS,
                &["check.yml: job check, step task check", word],
            );
        }
        fails(
            CHECK,
            &swap(
                DEPS,
                "        run: cargo test --workspace --locked --no-run\n",
                "        run: SCCACHE_GHA_ENABLED=true cargo test --workspace --locked --no-run\n",
            ),
            &[
                "deps-cache.yml: job deps, step Build the test targets",
                "SCCACHE_",
            ],
        );
    }

    #[test]
    fn a_missing_or_misplaced_restore_step_fails() {
        fails(
            &swap(
                CHECK,
                "      - name: Restore the dependency build\n",
                "      - name: Restore the build\n",
            ),
            DEPS,
            &["check.yml: job check has no step Restore the dependency build"],
        );
        let restore_start = CHECK
            .find("      - name: Restore the dependency build")
            .unwrap();
        let restore_end = CHECK.find("      - name: Fetch locked").unwrap();
        let restore = &CHECK[restore_start..restore_end];
        let without = CHECK.replacen(restore, "", 1);
        fails(
            &swap(
                &without,
                "      - name: task check\n",
                &format!("{restore}      - name: task check\n"),
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "before `cargo fetch --locked`",
            ],
        );
        fails(
            &swap(
                &without,
                "      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c\n        id: rust\n",
                &format!(
                    "{restore}      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c\n        id: rust\n"
                ),
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "after the step `id: rust`",
            ],
        );
        fails(
            &swap(
                CHECK,
                "'**/Cargo.toml') }}\n          restore-keys",
                "'**/Cargo.toml', 'x') }}\n          restore-keys",
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "not the key",
            ],
        );
        fails(
            &swap(CHECK, "            consumers/pinned/target\n", ""),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "paths",
            ],
        );
    }

    #[test]
    fn a_restore_or_cpu_step_without_the_class_condition_fails() {
        fails(
            &swap(
                CHECK,
                "      - name: Restore the dependency build\n        if: steps.changes.outputs.class != 'skip'\n",
                "      - name: Restore the dependency build\n",
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "steps.changes.outputs.class",
            ],
        );
        fails(
            &swap(
                CHECK,
                "      - name: Print the runner's CPU model\n        if: steps.changes.outputs.class != 'skip'\n",
                "      - name: Print the runner's CPU model\n        if: always()\n",
            ),
            DEPS,
            &[
                "check.yml: job check, step Print the runner's CPU model",
                "steps.changes.outputs.class",
            ],
        );
        fails(
            &swap(
                CHECK,
                "      - name: Print the runner's CPU model\n        if: steps.changes.outputs.class != 'skip'\n        run: lscpu | grep 'Model name'\n",
                "",
            ),
            DEPS,
            &["check.yml: job check has no step that runs `lscpu`"],
        );
    }

    #[test]
    fn a_job_reading_the_rustc_hash_without_id_rust_fails() {
        fails(
            &swap(CHECK, "        id: rust\n", ""),
            DEPS,
            &["check.yml: job check reads steps.rust.outputs", "id: rust"],
        );
        fails(
            CHECK,
            &swap(DEPS, "        id: rust\n", "        id: toolchain\n"),
            &[
                "deps-cache.yml: job deps reads steps.rust.outputs",
                "id: rust",
            ],
        );
    }

    #[test]
    fn the_writer_without_its_bounds_fails() {
        fails(
            CHECK,
            &swap(DEPS, "permissions:\n  contents: read\njobs:", "jobs:"),
            &[
                "deps-cache.yml: the workflow",
                "permissions: contents: read",
            ],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "    timeout-minutes: 40\n",
                "    timeout-minutes: 30\n",
            ),
            &["deps-cache.yml: job deps", "timeout-minutes: 40"],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "  cancel-in-progress: false\n",
                "  cancel-in-progress: true\n",
            ),
            &["deps-cache.yml: the workflow", "cancel-in-progress: false"],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "concurrency:\n  group: deps-cache\n  cancel-in-progress: false\n",
                "",
            ),
            &["deps-cache.yml: the workflow", "concurrency"],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "  group: deps-cache\n",
                "  group: deps-cache-${{ github.ref }}\n",
            ),
            &["deps-cache.yml: the workflow", "not per ref"],
        );
        let missing: Vec<(String, Value)> = vec![(
            "check.yml".to_owned(),
            serde_yaml_ng::from_str(CHECK).unwrap(),
        )];
        assert!(
            problems(&missing)
                .iter()
                .any(|p| p.contains("no deps-cache.yml")),
            "{:#?}",
            problems(&missing)
        );
    }

    #[test]
    fn a_key_without_the_image_the_rustc_hash_or_the_hashed_files_fails() {
        fails(
            &swap(
                CHECK,
                "    runs-on: ubuntu-24.04\n",
                "    runs-on: ubuntu-latest\n",
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "ubuntu-24.04",
            ],
        );
        for file in [
            "'rust-toolchain.toml', ",
            "'Cargo.lock', ",
            "'.cargo/config.toml', ",
        ] {
            let key = KEY_LINE.replacen(file, "", 1);
            fails(
                CHECK,
                &DEPS.replace(KEY_LINE, &key),
                &[
                    "deps-cache.yml: job deps, step Look the dependency build up",
                    file.trim_end_matches(", "),
                ],
            );
        }
        fails(
            CHECK,
            &DEPS.replace(KEY_LINE, &KEY_LINE.replacen(", '**/Cargo.toml'", "", 1)),
            &[
                "deps-cache.yml: job deps, step Restore an earlier dependency build",
                "'**/Cargo.toml'",
            ],
        );
        fails(
            CHECK,
            &DEPS.replace(
                KEY_LINE,
                &KEY_LINE.replacen("${{ steps.rust.outputs.cachekey }}-", "", 1),
            ),
            &[
                "deps-cache.yml: job deps, step Look the dependency build up",
                "steps.rust.outputs.cachekey",
            ],
        );
        fails(
            &swap(
                CHECK,
                "            loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-\n",
                "            loom-deps-ubuntu-24.04-\n",
            ),
            DEPS,
            &[
                "check.yml: job check, step Restore the dependency build",
                "restore-keys",
            ],
        );
        fails(
            CHECK,
            &DEPS.replace(
                KEY_LINE,
                &KEY_LINE.replacen("loom-deps-", "loom-dependencies-", 1),
            ),
            &["the keys differ", "check.yml", "deps-cache.yml"],
        );
        fails(
            CHECK,
            &swap(
                DEPS,
                "          key: ${{ steps.restore.outputs.cache-primary-key }}\n",
                "          key: loom-deps-ubuntu-24.04-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}\n",
            ),
            &[
                "deps-cache.yml: job deps, step Save the dependency build",
                "cache-primary-key",
            ],
        );
    }

    /// The tree's own workflows pass the guard.
    #[test]
    fn the_tree_s_workflows_hold_the_dependency_cache() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
        let mut workflows = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name.ends_with(".yml") || name.ends_with(".yaml") {
                let text = std::fs::read_to_string(&path).unwrap();
                let value =
                    serde_yaml_ng::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
                workflows.push((name, value));
            }
        }
        workflows.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(problems(&workflows), Vec::<String>::new());
    }
}
