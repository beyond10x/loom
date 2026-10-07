use crate::{
    MAX_PROTOCOL_BYTES, ProtocolCatalog, ProtocolDefinition, ProtocolSource, SourceKind, digest,
    registration_major,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// User-scoped protocol snapshots. Loading a catalog never accesses the source or the network.
#[derive(Debug, Clone)]
pub struct InstallStore {
    root: PathBuf,
}
impl InstallStore {
    /// Use an explicit installation directory (also useful for isolated hosts and tests).
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    /// Resolve `$XDG_DATA_HOME/loom/protocols`, or `$HOME/.local/share/loom/protocols`.
    pub fn user() -> Result<Self, String> {
        let root = if let Some(base) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            let base = PathBuf::from(base);
            if !base.is_absolute() {
                return Err("XDG_DATA_HOME must be absolute".into());
            }
            base
        } else {
            let home = std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .ok_or("HOME is unavailable")?;
            let home = PathBuf::from(home);
            if !home.is_absolute() {
                return Err("HOME must be absolute".into());
            }
            home.join(".local/share")
        };
        Ok(Self::new(root.join("loom/protocols")))
    }
    /// Validate every installed snapshot and compose it with the immutable bundled definitions.
    pub fn catalog(&self) -> Result<ProtocolCatalog, String> {
        let mut catalog = ProtocolCatalog::bundled()?;
        let files = match fs::read_dir(&self.root) {
            Ok(files) => files,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(catalog),
            Err(e) => return Err(format!("reading protocol installations: {e}")),
        };
        let mut paths = Vec::new();
        for file in files {
            let path = file.map_err(|e| e.to_string())?.path();
            // Atomic-write temporaries cannot become registrations until published.
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".install-"))
            {
                continue;
            }
            if path.extension().is_none_or(|e| e != "json") {
                return Err(format!(
                    "unexpected protocol installation entry: {}",
                    path.display()
                ));
            }
            paths.push(path);
        }
        paths.sort();
        for path in paths {
            let record = read_record(&path)?;
            if path.file_name().and_then(|p| p.to_str()) != Some(&format!("{}.json", record.name)) {
                return Err("installation filename does not match registration".into());
            }
            validate_installed_source(&record.source)?;
            if digest(record.yaml.as_bytes()) != record.sha256 {
                return Err(format!(
                    "{}: installed protocol digest mismatch",
                    record.name
                ));
            }
            catalog.add_yaml(&record.name, &record.yaml, record.source)?;
        }
        Ok(catalog)
    }
    /// Snapshot one local regular UTF-8 YAML file, validating before atomic publication.
    pub fn install_file(&self, name: &str, path: &Path, replace: bool) -> Result<(), String> {
        self.check_install(name, replace)?;
        let yaml = read_regular(path, MAX_PROTOCOL_BYTES)?;
        let location = path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        self.install(
            name,
            &yaml,
            ProtocolSource {
                kind: SourceKind::Local,
                location,
                revision: String::new(),
                path: String::new(),
            },
            replace,
        )
    }
    /// Fetch one regular blob from a full pinned Git commit without a checkout or executable package.
    pub fn install_git(
        &self,
        name: &str,
        source: &str,
        path: &str,
        replace: bool,
    ) -> Result<(), String> {
        self.check_install(name, replace)?;
        let (yaml, provenance) = crate::git::fetch(source, path)?;
        self.install(name, &yaml, provenance, replace)
    }
    /// Remove exactly one custom installation. Bundled definitions cannot be removed.
    pub fn remove(&self, name: &str) -> Result<(), String> {
        registration_major(name)?;
        if ProtocolCatalog::bundled()?.get(name).is_some() {
            return Err(format!("cannot remove bundled protocol {name}"));
        }
        fs::remove_file(self.root.join(format!("{name}.json")))
            .map_err(|e| format!("removing {name}: {e}"))
    }
    fn check_install(&self, name: &str, replace: bool) -> Result<(), String> {
        registration_major(name)?;
        if ProtocolCatalog::bundled()?.get(name).is_some() {
            return Err(format!("cannot replace bundled protocol {name}"));
        }
        if !replace
            && self
                .root
                .join(format!("{name}.json"))
                .symlink_metadata()
                .is_ok()
        {
            return Err(format!(
                "{name} is installed; replacement requires --replace"
            ));
        }
        Ok(())
    }
    fn install(
        &self,
        name: &str,
        yaml: &str,
        source: ProtocolSource,
        replace: bool,
    ) -> Result<(), String> {
        let mut validation = ProtocolCatalog::bundled()?;
        validation.add_yaml(name, yaml, source)?;
        let definition = &validation.get(name).expect("just inserted").definition;
        let bytes =
            serde_json::to_vec_pretty(&record_value(definition)).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".install-")
            .tempfile_in(&self.root)
            .map_err(|e| e.to_string())?;
        temporary.write_all(&bytes).map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        let destination = self.root.join(format!("{name}.json"));
        if replace {
            temporary.persist(destination)
        } else {
            temporary.persist_noclobber(destination)
        }
        .map_err(|e| format!("publishing protocol installation: {e}"))?;
        fs::File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|e| format!("syncing protocol installation directory: {e}"))?;
        Ok(())
    }
}
fn validate_installed_source(source: &ProtocolSource) -> Result<(), String> {
    match source.kind {
        SourceKind::Local
            if Path::new(&source.location).is_absolute()
                && source.revision.is_empty()
                && source.path.is_empty() =>
        {
            Ok(())
        }
        SourceKind::Git => {
            let (_, revision) = crate::git::validate_source(&source.location, &source.path)?;
            if revision != source.revision {
                return Err("installed Git provenance revision mismatch".into());
            }
            Ok(())
        }
        _ => Err("invalid installed protocol provenance".into()),
    }
}
fn record_value(record: &ProtocolDefinition) -> Value {
    json!({"format":"loom.protocol-install/1", "name":record.name,"yaml":record.yaml,"sha256":record.sha256,"source":{
        "kind":match record.source.kind { SourceKind::Local=>"local", SourceKind::Git=>"git", _=>unreachable!("installation sources are local or Git") },
        "location":record.source.location,"revision":record.source.revision,"path":record.source.path
    }})
}
fn read_record(path: &Path) -> Result<ProtocolDefinition, String> {
    let contents = read_regular(path, MAX_PROTOCOL_BYTES * 6 + 16384)?;
    let value: Value = serde_json::from_str(&contents)
        .map_err(|e| format!("corrupt protocol installation: {e}"))?;
    let object = value
        .as_object()
        .ok_or("protocol installation is not an object")?;
    if object.len() != 5 || field(&value, "format")? != "loom.protocol-install/1" {
        return Err("invalid protocol installation format or fields".into());
    }
    let source = value.get("source").ok_or("missing protocol source")?;
    if source.as_object().is_none_or(|s| s.len() != 4) {
        return Err("invalid protocol source fields".into());
    }
    Ok(ProtocolDefinition {
        name: field(&value, "name")?,
        yaml: field(&value, "yaml")?,
        sha256: field(&value, "sha256")?,
        source: ProtocolSource {
            kind: match field(source, "kind")?.as_str() {
                "local" => SourceKind::Local,
                "git" => SourceKind::Git,
                _ => return Err("unknown installed protocol source kind".into()),
            },
            location: field(source, "location")?,
            revision: field(source, "revision")?,
            path: field(source, "path")?,
        },
    })
}
fn field(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing or invalid installation field {key}"))
}
fn read_regular(path: &Path, max: usize) -> Result<String, String> {
    if !path
        .symlink_metadata()
        .map_err(|e| format!("reading {}: {e}", path.display()))?
        .file_type()
        .is_file()
    {
        return Err("protocol source must be a regular file, not a symlink".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("protocol file exceeds size limit".into());
    }
    String::from_utf8(bytes).map_err(|_| "protocol file is not UTF-8".into())
}
