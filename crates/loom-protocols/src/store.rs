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
    /// Validate the complete installed manifest and compose it with bundled definitions.
    /// A missing root is a first run; an existing root missing its manifest is corruption.
    pub fn catalog(&self) -> Result<ProtocolCatalog, String> {
        let mut catalog = ProtocolCatalog::bundled()?;
        for record in self.records()?.into_values() {
            catalog.add_yaml(&record.name, &record.yaml, record.source)?;
        }
        Ok(catalog)
    }
    fn records(&self) -> Result<std::collections::BTreeMap<String, ProtocolDefinition>, String> {
        if !checked_directory(&self.root)? {
            return Ok(Default::default());
        }
        read_manifest(&self.root.join("catalog.json"))
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
        self.check_name(name)?;
        if !checked_directory(&self.root)? {
            return Err(format!("{name} is not installed"));
        }
        let directory = fs::File::open(&self.root).map_err(|e| e.to_string())?;
        directory
            .lock()
            .map_err(|e| format!("locking protocol store: {e}"))?;
        let mut records = self.records()?;
        if records.remove(name).is_none() {
            return Err(format!("{name} is not installed"));
        }
        self.publish(records.values(), &directory)
    }
    fn check_name(&self, name: &str) -> Result<(), String> {
        registration_major(name)?;
        if ProtocolCatalog::bundled()?.get(name).is_some() {
            return Err(format!("cannot replace or remove bundled protocol {name}"));
        }
        Ok(())
    }
    fn check_install(&self, name: &str, replace: bool) -> Result<(), String> {
        self.check_name(name)?;
        let records = self.records()?;
        if !replace && records.contains_key(name) {
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
        let definition = validation
            .get(name)
            .expect("just inserted")
            .definition
            .clone();
        let existed = checked_directory(&self.root)?;
        let created = if existed {
            false
        } else {
            if let Some(parent) = self.root.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            checked_directory(&self.root)?;
            match fs::create_dir(&self.root) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
                Err(e) => return Err(e.to_string()),
            }
        };
        checked_directory(&self.root)?;
        // Serialize read-modify-publish transactions on the directory inode; catalog readers
        // see either complete manifest through the atomic rename, never a partial update.
        let directory = fs::File::open(&self.root).map_err(|e| e.to_string())?;
        directory
            .lock()
            .map_err(|e| format!("locking protocol store: {e}"))?;
        let mut records = if created {
            Default::default()
        } else {
            self.records()?
        };
        if !replace && records.contains_key(name) {
            return Err(format!(
                "{name} is installed; replacement requires --replace"
            ));
        }
        records.insert(name.into(), definition);
        self.publish(records.values(), &directory)
    }
    fn publish<'a>(
        &self,
        records: impl Iterator<Item = &'a ProtocolDefinition>,
        directory: &fs::File,
    ) -> Result<(), String> {
        checked_directory(&self.root)?;
        let records: Vec<_> = records.map(record_value).collect();
        let bytes = serde_json::to_vec_pretty(
            &json!({"format":"loom.protocol-catalog/1", "definitions":records}),
        )
        .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err("installed protocol catalog exceeds 64 MiB capacity".into());
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".install-")
            .tempfile_in(&self.root)
            .map_err(|e| e.to_string())?;
        temporary.write_all(&bytes).map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary
            .persist(self.root.join("catalog.json"))
            .map_err(|e| format!("publishing protocol catalog: {e}"))?;
        directory
            .sync_all()
            .map_err(|e| format!("syncing protocol installation directory: {e}"))?;
        Ok(())
    }
}

const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
fn read_manifest(
    path: &Path,
) -> Result<std::collections::BTreeMap<String, ProtocolDefinition>, String> {
    let contents = read_regular(path, MAX_MANIFEST_BYTES).map_err(|e| format!("protocol catalog missing or unreadable; restore catalog.json or remove an interrupted empty store: {e}"))?;
    let value: Value =
        serde_json::from_str(&contents).map_err(|e| format!("corrupt protocol catalog: {e}"))?;
    if value.as_object().is_none_or(|o| o.len() != 2)
        || field(&value, "format")? != "loom.protocol-catalog/1"
    {
        return Err("invalid protocol catalog manifest format".into());
    }
    let entries = value
        .get("definitions")
        .and_then(Value::as_array)
        .ok_or("missing protocol catalog definitions")?;
    let mut catalog = ProtocolCatalog::bundled()?;
    let mut records = std::collections::BTreeMap::new();
    for value in entries {
        let record = read_record(value)?;
        validate_installed_source(&record.source)?;
        if digest(record.yaml.as_bytes()) != record.sha256 {
            return Err(format!(
                "{}: installed protocol digest mismatch",
                record.name
            ));
        }
        catalog.add_yaml(&record.name, &record.yaml, record.source.clone())?;
        records.insert(record.name.clone(), record);
    }
    Ok(records)
}
/// Do not follow symlinks in any existing component, including dangling links. Missing
/// components describe a prospective first-install directory; other errors remain errors.
fn checked_directory(path: &Path) -> Result<bool, String> {
    use std::path::Component;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut prefix = PathBuf::new();
    let mut missing = false;
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                return Err("protocol store path cannot contain parent traversal".into());
            }
            Component::CurDir => continue,
            _ => prefix.push(component.as_os_str()),
        }
        match prefix.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("protocol store path and ancestors must not be symlinks".into());
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err("protocol store path and ancestors must be directories".into());
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => missing = true,
            Err(e) => return Err(format!("inspecting protocol store directory: {e}")),
        }
    }
    Ok(!missing)
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
fn read_record(value: &Value) -> Result<ProtocolDefinition, String> {
    let object = value
        .as_object()
        .ok_or("protocol installation is not an object")?;
    if object.len() != 5 || field(value, "format")? != "loom.protocol-install/1" {
        return Err("invalid protocol installation format or fields".into());
    }
    let source = value.get("source").ok_or("missing protocol source")?;
    if source.as_object().is_none_or(|s| s.len() != 4) {
        return Err("invalid protocol source fields".into());
    }
    Ok(ProtocolDefinition {
        name: field(value, "name")?,
        yaml: field(value, "yaml")?,
        sha256: field(value, "sha256")?,
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
