//! Minecraft resource context. Java/Kotlin models and live classpaths belong to the host.
use crate::mapping::{
    CompositeMappingResolver, EmptyMappingResolver, MappingResolver, ProjectMappingContext,
    SrgParser, TinyV2Parser,
};
use crate::MappingNamespace;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModPlatform {
    Fabric,
    Forge,
    #[serde(rename = "NEOFORGE")]
    NeoForge,
    #[default]
    Unknown,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectIndexState {
    #[default]
    NotReady,
    Building,
    Ready,
    Stale,
    Failed,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClasspathSnapshot {
    pub project_outputs: Vec<PathBuf>,
    pub dependency_jars: Vec<PathBuf>,
    pub minecraft_jars: Vec<PathBuf>,
    pub generated_outputs: Vec<PathBuf>,
    pub entry_timestamps: BTreeMap<PathBuf, u64>,
    pub captured_at: u64,
}
impl ClasspathSnapshot {
    pub fn all_entries(&self) -> Vec<PathBuf> {
        [
            &self.project_outputs,
            &self.dependency_jars,
            &self.minecraft_jars,
            &self.generated_outputs,
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
    }
    pub fn entry_count(&self) -> usize {
        self.project_outputs.len()
            + self.dependency_jars.len()
            + self.minecraft_jars.len()
            + self.generated_outputs.len()
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSetContext {
    pub name: String,
    pub source_directories: Vec<PathBuf>,
    pub resource_directories: Vec<PathBuf>,
    pub output_directory: Option<PathBuf>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinConfigRef {
    pub path: PathBuf,
    pub package_name: Option<String>,
    pub mixins: Vec<String>,
    pub client: Vec<String>,
    pub server: Vec<String>,
    pub common: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceIssue {
    pub path: PathBuf,
    pub message: String,
}

/// Stable host facts are inputs, never independently re-resolved from Java/Kotlin source.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostProjectFacts {
    pub classpath: ClasspathSnapshot,
    pub source_sets: Vec<SourceSetContext>,
    pub project_generation: u64,
    pub dependency_generation: u64,
    pub generated_source_generation: u64,
    pub config_generation: u64,
    pub provider_generation: u64,
}

pub struct PlatformDetector;
impl PlatformDetector {
    pub fn detect(contents: &[String]) -> ModPlatform {
        let s = contents.join("\n").to_lowercase();
        if s.contains("fabric-loom") {
            ModPlatform::Fabric
        } else if s.contains("net.neoforged.gradle")
            || s.contains("org.neoforged.moddev")
            || (s.contains("neoforge") && s.contains("moddev"))
        {
            ModPlatform::NeoForge
        } else if s.contains("forgegradle")
            || s.contains("net.minecraftforge.gradle")
            || s.contains("minecraftforge")
        {
            ModPlatform::Forge
        } else {
            ModPlatform::Unknown
        }
    }
    pub fn detect_from_root(root: &Path) -> io::Result<ModPlatform> {
        Ok(Self::detect(&read_gradle_contents(root)?))
    }
}
fn read_gradle_contents(root: &Path) -> io::Result<Vec<String>> {
    [
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ]
    .into_iter()
    .filter_map(|name| {
        let path = root.join(name);
        path.is_file().then(|| fs::read_to_string(path))
    })
    .collect()
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            part => out.push(part.as_os_str()),
        }
    }
    out
}
fn excluded_segment(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "build" | ".gradle" | "node_modules"
    )
}
const GRADLE_PREFIXES: &[&[&str]] = &[
    &["loom-cache"],
    &["caches", "fabric-loom"],
    &["caches", "modules-2", "files-2.1", "net.fabricmc", "yarn"],
    &["caches", "modules-2", "files-2.1", "net.minecraft"],
    &["caches", "modules-2", "files-2.1", "net.minecraftforge"],
    &["caches", "modules-2", "files-2.1", "net.neoforged"],
];
const BUILD_PREFIXES: &[&[&str]] = &[&["loom-cache"], &["createSrgToMcp"], &["tmp"]];
pub fn is_under_excluded_directory(path: &Path) -> bool {
    path.components()
        .any(|c| excluded_segment(&c.as_os_str().to_string_lossy()))
}
fn mapping_excluded(path: &Path, partial: bool) -> bool {
    let parts = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    for (i, part) in parts.iter().enumerate() {
        let prefixes = match part.as_str() {
            "node_modules" => return true,
            ".gradle" => GRADLE_PREFIXES,
            "build" => BUILD_PREFIXES,
            _ => continue,
        };
        let suffix = &parts[i + 1..];
        if partial && suffix.is_empty() {
            continue;
        }
        if !prefixes.iter().any(|prefix| {
            (partial || suffix.len() >= prefix.len())
                && suffix
                    .iter()
                    .zip(prefix.iter())
                    .all(|(a, b)| a.eq_ignore_ascii_case(b))
        }) {
            return true;
        }
    }
    false
}
pub fn is_excluded_from_mapping_discovery(path: &Path) -> bool {
    mapping_excluded(path, false)
}
pub fn should_prune_mapping_discovery_directory(path: &Path) -> bool {
    mapping_excluded(path, true)
}
pub fn walk_regular_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    walk_files(root, false)
}
pub fn walk_mapping_discovery_regular_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    walk_files(&normalize_path(root), true)
}
fn walk_files(root: &Path, mappings: bool) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    fn visit(root: &Path, dir: &Path, mappings: bool, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                let excluded = if mappings {
                    should_prune_mapping_discovery_directory(
                        path.strip_prefix(root).unwrap_or(&path),
                    )
                } else {
                    excluded_segment(&entry.file_name().to_string_lossy())
                };
                if !excluded {
                    visit(root, &path, mappings, out)?;
                }
            } else if kind.is_file() {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut out = vec![];
    visit(root, root, mappings, &mut out)?;
    // Windows Path.compareTo, used by the original walker, compares complete
    // paths without case sensitivity rather than component lengths.
    out.sort_by_key(|path| {
        let text = path.to_string_lossy().into_owned();
        if cfg!(windows) {
            text.replace('/', "\\").to_uppercase()
        } else {
            text
        }
    });
    Ok(out)
}

pub struct SourceSetResolver;
impl SourceSetResolver {
    pub fn containing_source_set<'a>(
        sets: &'a [SourceSetContext],
        document: &Path,
    ) -> Option<&'a SourceSetContext> {
        let document = normalize_path(document);
        let mut matches = Vec::new();
        for (i, set) in sets.iter().enumerate() {
            for dir in &set.source_directories {
                let dir = normalize_path(dir);
                if document.starts_with(&dir) {
                    matches.push((dir.components().count(), i));
                }
            }
        }
        let depth = matches.iter().map(|(d, _)| *d).max()?;
        let indices = matches
            .iter()
            .filter(|(d, _)| *d == depth)
            .map(|(_, i)| *i)
            .collect::<BTreeSet<_>>();
        (indices.len() == 1).then(|| &sets[*indices.first().unwrap()])
    }
}

pub struct MixinConfigDiscoveryService;
impl MixinConfigDiscoveryService {
    pub fn is_mixin_config_file(path: &Path) -> bool {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        name.contains("mixin") && (name.ends_with(".json") || name.ends_with(".json5"))
    }
    pub fn discover(root: &Path) -> io::Result<Vec<MixinConfigRef>> {
        Ok(walk_regular_files(root)?
            .into_iter()
            .filter(|p| Self::is_mixin_config_file(p))
            .filter_map(|p| Self::parse(&p))
            .collect())
    }
    pub fn parse(path: &Path) -> Option<MixinConfigRef> {
        Self::parse_content(path, &fs::read_to_string(path).ok()?)
    }
    pub fn parse_content(path: &Path, content: &str) -> Option<MixinConfigRef> {
        let value = crate::mixin_edits::parse_config_json(content).ok()?;
        let obj = value.as_object()?;
        let strings = |key: &str| {
            obj.get(key)
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        Some(MixinConfigRef {
            path: path.into(),
            package_name: obj
                .get("package")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            mixins: strings("mixins"),
            client: strings("client"),
            server: strings("server"),
            common: strings("common"),
        })
    }
    pub fn normalize_json_content(content: &str) -> String {
        let chars = content.chars().collect::<Vec<_>>();
        let mut stripped = String::new();
        let mut i = 0;
        let mut quote = None;
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = quote {
                stripped.push(c);
                if c == '\\' && i + 1 < chars.len() {
                    i += 1;
                    stripped.push(chars[i]);
                } else if c == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            match c {
                '\'' | '"' => {
                    quote = Some(c);
                    stripped.push(c);
                    i += 1;
                }
                '/' if chars.get(i + 1) == Some(&'/') => {
                    i += 2;
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                }
                '/' if chars.get(i + 1) == Some(&'*') => {
                    i += 2;
                    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                        if chars[i] == '\n' {
                            stripped.push('\n');
                        }
                        i += 1;
                    }
                    i = (i + 2).min(chars.len());
                }
                _ => {
                    stripped.push(c);
                    i += 1;
                }
            }
        }
        let chars = stripped.chars().collect::<Vec<_>>();
        let mut out = String::new();
        let mut i = 0;
        let mut quote = None;
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = quote {
                out.push(c);
                if c == '\\' && i + 1 < chars.len() {
                    i += 1;
                    out.push(chars[i]);
                } else if c == q {
                    quote = None;
                }
            } else if c == '\'' || c == '"' {
                quote = Some(c);
                out.push(c);
            } else if c != ','
                || !chars[i + 1..]
                    .iter()
                    .find(|c| !c.is_whitespace())
                    .is_some_and(|c| *c == '}' || *c == ']')
            {
                out.push(c);
            }
            i += 1;
        }
        out
    }
    pub fn shared_configs(
        configs: &[MixinConfigRef],
        sets: &[SourceSetContext],
        root: &Path,
    ) -> Vec<MixinConfigRef> {
        let root = normalize_path(root);
        let mut result = configs
            .iter()
            .filter(|c| {
                let path = normalize_path(&c.path);
                path.starts_with(&root)
                    && !sets
                        .iter()
                        .flat_map(|s| &s.resource_directories)
                        .any(|d| path.starts_with(normalize_path(d)))
            })
            .cloned()
            .collect::<Vec<_>>();
        result.sort_by_key(|c| normalize_path(&c.path));
        result
    }
    pub fn configs_for_source_set(
        configs: &[MixinConfigRef],
        set: &SourceSetContext,
        sets: &[SourceSetContext],
        root: &Path,
    ) -> Vec<MixinConfigRef> {
        let root = normalize_path(root);
        let mut local = configs
            .iter()
            .filter(|c| {
                let path = normalize_path(&c.path);
                path.starts_with(&root)
                    && set
                        .resource_directories
                        .iter()
                        .any(|d| path.starts_with(normalize_path(d)))
            })
            .cloned()
            .collect::<Vec<_>>();
        local.sort_by_key(|c| normalize_path(&c.path));
        if local.is_empty() {
            Self::shared_configs(configs, sets, &root)
        } else {
            local
        }
    }
    pub fn is_mixin_listed(config: &MixinConfigRef, class: &str, package: Option<&str>) -> bool {
        let qualified = package.map_or_else(|| class.to_owned(), |p| format!("{p}.{class}"));
        config
            .mixins
            .iter()
            .chain(&config.client)
            .chain(&config.server)
            .chain(&config.common)
            .any(|entry| {
                let pkg = config
                    .package_name
                    .as_deref()
                    .filter(|p| !p.trim().is_empty());
                let resolved = pkg.map_or_else(|| entry.clone(), |p| format!("{p}.{entry}"));
                resolved == qualified || (pkg.is_none() && entry == class)
            })
    }
    pub fn select_for_mixin<'a>(
        configs: &'a [MixinConfigRef],
        class: Option<&str>,
        package: Option<&str>,
    ) -> Option<&'a MixinConfigRef> {
        let qualified = match (package, class) {
            (Some(p), Some(c)) => format!("{p}.{c}"),
            (Some(p), None) => p.into(),
            (None, Some(c)) => c.into(),
            _ => return None,
        };
        if let Some(class) = class {
            let listed = configs
                .iter()
                .filter(|c| Self::is_mixin_listed(c, class, package))
                .collect::<Vec<_>>();
            if listed.len() == 1 {
                return Some(listed[0]);
            }
            if listed.len() > 1 {
                return None;
            }
        }
        package?;
        let candidates = configs
            .iter()
            .filter(|c| {
                c.package_name.as_ref().is_some_and(|p| {
                    !p.trim().is_empty()
                        && (qualified == *p || qualified.starts_with(&format!("{p}.")))
                })
            })
            .collect::<Vec<_>>();
        let longest = candidates
            .iter()
            .map(|c| c.package_name.as_ref().unwrap().len())
            .max()?;
        let best = candidates
            .into_iter()
            .filter(|c| c.package_name.as_ref().unwrap().len() == longest)
            .collect::<Vec<_>>();
        (best.len() == 1).then(|| best[0])
    }
}

pub struct MappingDiscoveryService;
impl MappingDiscoveryService {
    pub fn parse_mapping_file(path: &Path) -> io::Result<crate::mapping::MappingSet> {
        let text = fs::read_to_string(path)?;
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let parsed = if ext == "tiny" || text.trim_start().starts_with("tiny\t") {
            TinyV2Parser::parse(&text)
        } else if ext == "srg" || ext == "tsrg" || text.contains("CL:") || text.contains("MD:") {
            SrgParser::parse(&text)
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown mapping format",
            ));
        };
        parsed.map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: {}", e.line, e.message),
            )
        })
    }
    pub fn discover_mapping_context(
        root: &Path,
        platform: ModPlatform,
    ) -> io::Result<ProjectMappingContext> {
        Self::build_mapping_context(&Self::discover_mapping_files(root)?, platform)
    }
    pub fn discover_mapping_files(root: &Path) -> io::Result<Vec<PathBuf>> {
        let root = normalize_path(root);
        Ok(walk_mapping_discovery_regular_files(&root)?
            .into_iter()
            .filter(|p| {
                matches!(
                    p.extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .as_str(),
                    "tiny" | "srg" | "tsrg"
                ) && !is_excluded_from_mapping_discovery(p.strip_prefix(&root).unwrap_or(p))
            })
            .collect())
    }
    pub fn build_mapping_context(
        files: &[PathBuf],
        platform: ModPlatform,
    ) -> io::Result<ProjectMappingContext> {
        let mut namespaces = vec![];
        let mut resolvers = Vec::<Arc<dyn MappingResolver>>::new();
        for path in files {
            let text = fs::read_to_string(path)?;
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            let parsed = if ext == "tiny" || text.trim_start().starts_with("tiny\t") {
                TinyV2Parser::parse(&text)
            } else {
                SrgParser::parse(&text)
            };
            if let Ok(set) = parsed {
                for ns in &set.namespaces {
                    if !namespaces.contains(ns) {
                        namespaces.push(*ns);
                    }
                }
                resolvers.push(Arc::new(set.as_resolver()));
            }
        }
        if resolvers.is_empty() {
            return Ok(empty_mapping_context());
        }
        let choose = |choices: &[MappingNamespace]| {
            choices
                .iter()
                .find(|ns| namespaces.contains(ns))
                .copied()
                .unwrap_or(namespaces[0])
        };
        use MappingNamespace::*;
        let source = match platform {
            ModPlatform::Fabric => choose(&[Named, Intermediary]),
            ModPlatform::Forge | ModPlatform::NeoForge => choose(&[Mcp, Srg, Named]),
            ModPlatform::Unknown => namespaces[0],
        };
        let runtime = match platform {
            ModPlatform::Fabric => choose(&[Intermediary, Official]),
            ModPlatform::Forge | ModPlatform::NeoForge => choose(&[Srg, Official]),
            ModPlatform::Unknown => namespaces
                .iter()
                .find(|ns| **ns != source)
                .copied()
                .unwrap_or(source),
        };
        let resolver = CompositeMappingResolver::new(resolvers).map_err(io::Error::other)?;
        Ok(ProjectMappingContext {
            source_namespace: source,
            runtime_namespace: runtime,
            aw_namespace: Some(choose(&[Named])),
            at_namespace: Some(choose(&[Srg])),
            available_namespaces: namespaces.into_iter().collect(),
            resolver: Arc::new(resolver),
        })
    }
}
pub fn empty_mapping_context() -> ProjectMappingContext {
    ProjectMappingContext {
        source_namespace: MappingNamespace::Named,
        runtime_namespace: MappingNamespace::Intermediary,
        aw_namespace: None,
        at_namespace: None,
        available_namespaces: BTreeSet::new(),
        resolver: Arc::new(EmptyMappingResolver),
    }
}

/// File URI conversion is UTF-8 percent decoding, not form decoding (`+` stays `+`).
pub fn uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let uri = uri.trim();
    if uri.is_empty() {
        return Err("workspace URI is empty".into());
    }
    if !uri.starts_with("file:") {
        if uri.contains("://") {
            return Err("workspace URI must use file scheme".into());
        }
        return Ok(PathBuf::from(uri));
    }
    let mut body = uri.strip_prefix("file://").ok_or("invalid file URI")?;
    if body.starts_with("localhost/") {
        body = &body["localhost".len()..];
    } else if !body.starts_with('/') {
        body = uri.strip_prefix("file:").unwrap();
    }
    if body.contains('?') || body.contains('#') {
        return Err("file URI cannot have query or fragment".into());
    }
    let bytes = body.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = body.get(i + 1..i + 3).ok_or("incomplete URI escape")?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| "invalid URI escape")?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let mut value = String::from_utf8(out).map_err(|_| "file URI is not UTF-8")?;
    if value.contains('\0') {
        return Err("file URI contains NUL".into());
    }
    if value.starts_with('/')
        && value.as_bytes().get(2) == Some(&b':')
        && value.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic)
    {
        value.remove(0);
    }
    Ok(value.into())
}
pub fn path_to_uri(path: &Path) -> String {
    let mut path = normalize_path(path).to_string_lossy().replace('\\', "/");
    // shortcut: only verbatim drive/UNC file paths are normalized, extend for device namespaces if LSP accepts them.
    if path
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("//?/UNC/"))
    {
        path.replace_range(..8, "//");
    } else if path.starts_with("//?/")
        && path.get(4..7).is_some_and(|prefix| {
            let bytes = prefix.as_bytes();
            bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
        })
    {
        path.replace_range(..4, "");
    }
    let mut encoded = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(b));
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    if encoded.starts_with("//") {
        format!("file:{encoded}")
    } else if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessWidenerRef {
    pub path: PathBuf,
    pub namespace: Option<MappingNamespace>,
    pub parsed: Option<crate::aw::AccessWidenerFile>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessTransformerRef {
    pub path: PathBuf,
    pub parsed: Option<crate::at::AccessTransformerFile>,
}
pub struct AwAtDiscoveryService;
impl AwAtDiscoveryService {
    pub fn is_access_widener_file(path: &Path) -> bool {
        matches!(
            path.extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
            "aw" | "accesswidener"
        )
    }
    pub fn is_access_transformer_file(path: &Path) -> bool {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        name == "accesstransformer.cfg"
            || name.ends_with("_at.cfg")
            || name.ends_with(".at")
            || (name.ends_with(".cfg") && name.contains("access") && name.contains("transform"))
    }
    pub fn parse_access_widener(path: &Path) -> io::Result<AccessWidenerRef> {
        let text = fs::read_to_string(path)?;
        let parsed = crate::aw::AccessWidenerParser::parse(&text).ok();
        Ok(AccessWidenerRef {
            path: path.into(),
            namespace: parsed.as_ref().map(|p| p.namespace),
            parsed,
        })
    }
    pub fn parse_access_transformer(path: &Path) -> io::Result<AccessTransformerRef> {
        let text = fs::read_to_string(path)?;
        Ok(AccessTransformerRef {
            path: path.into(),
            parsed: crate::at::AccessTransformerParser::parse(&text).ok(),
        })
    }
    pub fn discover_access_wideners(root: &Path) -> io::Result<Vec<AccessWidenerRef>> {
        walk_regular_files(root)?
            .iter()
            .filter(|p| Self::is_access_widener_file(p))
            .map(|p| Self::parse_access_widener(p))
            .collect()
    }
    pub fn discover_access_transformers(root: &Path) -> io::Result<Vec<AccessTransformerRef>> {
        walk_regular_files(root)?
            .iter()
            .filter(|p| Self::is_access_transformer_file(p))
            .map(|p| Self::parse_access_transformer(p))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub project_id: String,
    pub root: PathBuf,
    pub platform: ModPlatform,
    pub classpath: ClasspathSnapshot,
    pub mappings: ProjectMappingContext,
    pub mixin_configs: Vec<MixinConfigRef>,
    pub access_wideners: Vec<AccessWidenerRef>,
    pub access_transformers: Vec<AccessTransformerRef>,
    pub minecraft_jars: Vec<PathBuf>,
    pub source_sets: Vec<SourceSetContext>,
    pub index_state: ProjectIndexState,
    pub mapping_files: Vec<PathBuf>,
    pub resource_issues: Vec<ResourceIssue>,
}
#[derive(Debug, Clone)]
pub struct ProjectContextInput {
    pub project_id: String,
    pub root: PathBuf,
    pub platform: Option<ModPlatform>,
    pub gradle_contents: Vec<String>,
    pub classpath: ClasspathSnapshot,
    pub mappings: Option<ProjectMappingContext>,
    pub mixin_configs: Option<Vec<MixinConfigRef>>,
    pub access_wideners: Option<Vec<AccessWidenerRef>>,
    pub access_transformers: Option<Vec<AccessTransformerRef>>,
    pub minecraft_jars: Option<Vec<PathBuf>>,
    pub source_sets: Vec<SourceSetContext>,
    pub index_state: ProjectIndexState,
}
impl ProjectContextInput {
    pub fn new(project_id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            project_id: project_id.into(),
            root: root.into(),
            platform: None,
            gradle_contents: vec![],
            classpath: ClasspathSnapshot::default(),
            mappings: None,
            mixin_configs: None,
            access_wideners: None,
            access_transformers: None,
            minecraft_jars: None,
            source_sets: vec![],
            index_state: ProjectIndexState::NotReady,
        }
    }
}
pub struct ProjectContextBuilder;
impl ProjectContextBuilder {
    pub fn build(input: ProjectContextInput) -> io::Result<ProjectContext> {
        let platform = if let Some(p) = input.platform {
            p
        } else if input.gradle_contents.is_empty() {
            PlatformDetector::detect_from_root(&input.root)?
        } else {
            PlatformDetector::detect(&input.gradle_contents)
        };
        let mapping_files = if input.mappings.is_none() {
            MappingDiscoveryService::discover_mapping_files(&input.root)?
        } else {
            vec![]
        };
        let mappings = if let Some(m) = input.mappings {
            m
        } else {
            MappingDiscoveryService::build_mapping_context(&mapping_files, platform)?
        };
        let mut resource_issues = vec![];
        let mixin_configs = if let Some(configs) = input.mixin_configs {
            configs
        } else {
            let mut configs = vec![];
            for path in walk_regular_files(&input.root)?
                .into_iter()
                .filter(|p| MixinConfigDiscoveryService::is_mixin_config_file(p))
            {
                if let Some(config) = MixinConfigDiscoveryService::parse(&path) {
                    configs.push(config);
                } else {
                    resource_issues.push(ResourceIssue {
                        path,
                        message: "Invalid or unreadable Mixin config".into(),
                    });
                }
            }
            configs
        };
        let access_wideners = if let Some(aw) = input.access_wideners {
            aw
        } else {
            AwAtDiscoveryService::discover_access_wideners(&input.root)?
        };
        let access_transformers = if let Some(at) = input.access_transformers {
            at
        } else {
            AwAtDiscoveryService::discover_access_transformers(&input.root)?
        };
        Ok(ProjectContext {
            project_id: input.project_id,
            root: input.root,
            platform,
            minecraft_jars: input
                .minecraft_jars
                .unwrap_or_else(|| input.classpath.minecraft_jars.clone()),
            classpath: input.classpath,
            mappings,
            mixin_configs,
            access_wideners,
            access_transformers,
            source_sets: input.source_sets,
            index_state: input.index_state,
            mapping_files,
            resource_issues,
        })
    }
    pub fn empty(project_id: impl Into<String>, root: impl Into<PathBuf>) -> ProjectContext {
        ProjectContext {
            project_id: project_id.into(),
            root: root.into(),
            platform: ModPlatform::Unknown,
            classpath: ClasspathSnapshot::default(),
            mappings: empty_mapping_context(),
            mixin_configs: vec![],
            access_wideners: vec![],
            access_transformers: vec![],
            minecraft_jars: vec![],
            source_sets: vec![],
            index_state: ProjectIndexState::NotReady,
            mapping_files: vec![],
            resource_issues: vec![],
        }
    }
}

/// Explicit file fallback for clients without host project facts; never used to override supplied facts.
pub fn discover_source_sets(root: &Path) -> io::Result<Vec<SourceSetContext>> {
    let src = root.join("src");
    if !src.is_dir() {
        return Ok(vec![]);
    }
    let mut names = fs::read_dir(&src)?
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name())
        .collect::<Vec<_>>();
    names.sort();
    let mut sets = vec![];
    for name in names {
        let set = src.join(&name);
        let java = set.join("java");
        let kotlin = set.join("kotlin");
        let resources = set.join("resources");
        if !java.is_dir() && !kotlin.is_dir() && !resources.is_dir() {
            continue;
        }
        let name = name.to_string_lossy().into_owned();
        let mut dirs = [java.clone(), kotlin.clone()]
            .into_iter()
            .filter(|p| p.is_dir())
            .collect::<Vec<_>>();
        if name == "main" && root.join("mapped-sources").is_dir() {
            dirs.push(root.join("mapped-sources"));
        }
        let language = if kotlin.is_dir() && !java.is_dir() {
            "kotlin"
        } else {
            "java"
        };
        sets.push(SourceSetContext {
            name: name.clone(),
            source_directories: dirs,
            resource_directories: resources
                .is_dir()
                .then_some(resources)
                .into_iter()
                .collect(),
            output_directory: Some(root.join(format!("build/classes/{language}/{name}"))),
        })
    }
    Ok(sets)
}
fn timestamp(path: &Path) -> io::Result<u64> {
    Ok(fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64)
}
fn all_files_unfiltered(root: &Path) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    Ok(out)
}
pub fn discover_classpath(root: &Path, sets: &[SourceSetContext]) -> io::Result<ClasspathSnapshot> {
    let mut snapshot = ClasspathSnapshot {
        captured_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        ..Default::default()
    };
    for set in sets {
        for lang in ["java", "kotlin"] {
            let dir = root.join(format!("build/classes/{lang}/{}", set.name));
            if dir.is_dir() {
                snapshot.project_outputs.push(dir);
            }
        }
    }
    if snapshot.project_outputs.is_empty() && root.join("build/classes").is_dir() {
        snapshot.project_outputs.push(root.join("build/classes"));
    }
    for relative in ["bin/main", "out/production/classes", "classpath"] {
        let dir = root.join(relative);
        if dir.is_dir() {
            snapshot.project_outputs.push(dir);
        }
    }
    for relative in ["build/generated/sources", "build/generated/resources"] {
        let base = root.join(relative);
        if !base.is_dir() {
            continue;
        }
        let mut pending = vec![base.clone()];
        while let Some(dir) = pending.pop() {
            let children = fs::read_dir(&dir)?
                .map(|e| e.and_then(|e| Ok((e.file_type()?, e.path()))))
                .collect::<io::Result<Vec<_>>>()?
                .into_iter()
                .filter(|(kind, _)| kind.is_dir())
                .map(|(_, path)| path)
                .collect::<Vec<_>>();
            if children.is_empty() && dir != base {
                snapshot.generated_outputs.push(dir);
            } else {
                pending.extend(children);
            }
        }
    }
    for relative in [
        "libs",
        "build/libs",
        "run/mods",
        ".gradle/loom-cache/remapped_working",
        "build/loom-cache/remapped_working",
    ] {
        for path in all_files_unfiltered(&root.join(relative))?
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")))
        {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if name.contains("minecraft") || (name.contains("client") && name.contains("mapped")) {
                snapshot.minecraft_jars.push(path);
            } else {
                snapshot.dependency_jars.push(path);
            }
        }
    }
    snapshot.project_outputs.sort();
    snapshot.project_outputs.dedup();
    snapshot.generated_outputs.sort();
    snapshot.dependency_jars.sort();
    snapshot.dependency_jars.dedup();
    snapshot.minecraft_jars.sort();
    snapshot.minecraft_jars.dedup();
    for path in snapshot.all_entries() {
        snapshot
            .entry_timestamps
            .insert(path.clone(), timestamp(&path)?);
    }
    Ok(snapshot)
}

pub fn build_file_project_context(
    root: &Path,
    facts: Option<&HostProjectFacts>,
) -> io::Result<ProjectContext> {
    let mut input =
        ProjectContextInput::new(root.file_name().unwrap_or_default().to_string_lossy(), root);
    if let Some(facts) = facts {
        input.classpath = facts.classpath.clone();
        input.source_sets = facts.source_sets.clone();
    } else {
        input.source_sets = discover_source_sets(root)?;
        input.classpath = discover_classpath(root, &input.source_sets)?;
    }
    input.index_state = if input.classpath.entry_count() > 0 {
        ProjectIndexState::Ready
    } else {
        ProjectIndexState::NotReady
    };
    ProjectContextBuilder::build(input)
}

pub struct InfoService;
impl InfoService {
    pub fn namespace_name(namespace: MappingNamespace) -> &'static str {
        match namespace {
            MappingNamespace::Named => "named",
            MappingNamespace::Intermediary => "intermediary",
            MappingNamespace::Official => "official",
            MappingNamespace::Srg => "srg",
            MappingNamespace::Mcp => "mcp",
        }
    }
    pub fn index_state_name(state: ProjectIndexState) -> &'static str {
        match state {
            ProjectIndexState::NotReady => "not_ready",
            ProjectIndexState::Building => "building",
            ProjectIndexState::Ready => "ready",
            ProjectIndexState::Stale => "stale",
            ProjectIndexState::Failed => "failed",
        }
    }
    pub fn format_lines(
        context: &ProjectContext,
        protocol_version: u32,
        extension_version: &str,
    ) -> Vec<String> {
        let platform = match context.platform {
            ModPlatform::Fabric => "Fabric",
            ModPlatform::Forge => "Forge",
            ModPlatform::NeoForge => "NeoForge",
            ModPlatform::Unknown => "Unknown",
        };
        let ns = |ns: MappingNamespace| match ns {
            MappingNamespace::Named => "named",
            MappingNamespace::Intermediary => "intermediary",
            MappingNamespace::Official => "official",
            MappingNamespace::Srg => "srg",
            MappingNamespace::Mcp => "mcp",
        };
        let count = |label: &str, n: usize| match n {
            0 => format!("{label}: none"),
            1 => format!("{label}: 1 file"),
            _ => format!("{label}: {n} files"),
        };
        let available = &context.mappings.available_namespaces;
        let state = match context.index_state {
            ProjectIndexState::NotReady => "not ready",
            ProjectIndexState::Building => "building",
            ProjectIndexState::Ready => "ready",
            ProjectIndexState::Stale => "stale",
            ProjectIndexState::Failed => "failed",
        };
        vec![
            format!("Project: {}", context.project_id),
            format!("Root: {}", context.root.display()),
            format!("Platform: {platform}"),
            if available.is_empty() {
                "Mappings: none".into()
            } else {
                format!(
                    "Mappings: {} <-> {} {}",
                    ns(context.mappings.source_namespace),
                    ns(context.mappings.runtime_namespace),
                    if available.len() >= 2 {
                        "loaded"
                    } else {
                        "partial"
                    }
                )
            },
            if !context.mapping_files.is_empty() {
                format!("Mapping source: {} file(s)", context.mapping_files.len())
            } else if available.is_empty() {
                "Mapping source: none (no supported mapping files discovered)".into()
            } else {
                "Mapping source: provided".into()
            },
            format!(
                "Source namespace: {}",
                ns(context.mappings.source_namespace)
            ),
            format!(
                "Runtime namespace: {}",
                ns(context.mappings.runtime_namespace)
            ),
            format!(
                "Minecraft jar: {}",
                if !context.minecraft_jars.is_empty()
                    || !context.classpath.minecraft_jars.is_empty()
                {
                    "found"
                } else {
                    "none"
                }
            ),
            count("Mixin config", context.mixin_configs.len()),
            if context.mixin_configs.is_empty() {
                format!(
                    "Mixin config search: none found under {}",
                    context.root.display()
                )
            } else {
                format!(
                    "Mixin config search: {}",
                    context
                        .mixin_configs
                        .iter()
                        .map(|c| c.path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            count("Access Widener", context.access_wideners.len()),
            count("Access Transformer", context.access_transformers.len()),
            format!("Classpath entries: {}", context.classpath.entry_count()),
            if context.classpath.entry_count() > 0 {
                format!(
                    "Classpath source: project outputs={}, dependencies={}, minecraft={}",
                    context.classpath.project_outputs.len(),
                    context.classpath.dependency_jars.len(),
                    context.classpath.minecraft_jars.len()
                )
            } else {
                "Classpath source: empty (no project outputs, dependency jars, or minecraft jars discovered)".into()
            },
            format!("Source sets: {}", context.source_sets.len()),
            format!("Class index: {state}"),
            format!("Bytecode index: {state}"),
            if available.is_empty() {
                "Mapping namespaces: none (searched project mapping files and known Gradle mapping caches)".into()
            } else {
                format!(
                    "Mapping namespaces: {}",
                    available
                        .iter()
                        .map(|n| ns(*n))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            format!("Protocol: {protocol_version}"),
            format!("Extension: {extension_version}"),
        ]
    }
    pub fn format(
        context: &ProjectContext,
        protocol_version: u32,
        extension_version: &str,
    ) -> String {
        Self::format_lines(context, protocol_version, extension_version).join("\n")
    }
    pub fn dump(context: &ProjectContext) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&Self::dump_value(context, 1, "0.1.0"))
    }
    pub fn dump_value(
        context: &ProjectContext,
        protocol_version: u32,
        extension_version: &str,
    ) -> serde_json::Value {
        let platform = match context.platform {
            ModPlatform::Fabric => "fabric",
            ModPlatform::Forge => "forge",
            ModPlatform::NeoForge => "neoforge",
            ModPlatform::Unknown => "unknown",
        };
        let paths = |paths: &[PathBuf]| {
            paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        let mut namespaces = context
            .mappings
            .available_namespaces
            .iter()
            .map(|n| Self::namespace_name(*n))
            .collect::<Vec<_>>();
        namespaces.sort();
        serde_json::json!({
            "lines":Self::format_lines(context,protocol_version,extension_version),"projectId":context.project_id,"root":context.root,"platform":platform,
            "mappings":{"sourceNamespace":Self::namespace_name(context.mappings.source_namespace),"runtimeNamespace":Self::namespace_name(context.mappings.runtime_namespace),"awNamespace":context.mappings.aw_namespace.map(Self::namespace_name),"atNamespace":context.mappings.at_namespace.map(Self::namespace_name),"availableNamespaces":namespaces},
            "classpath":{"entryCount":context.classpath.entry_count(),"projectOutputs":paths(&context.classpath.project_outputs),"dependencyJars":paths(&context.classpath.dependency_jars),"minecraftJars":paths(&context.classpath.minecraft_jars),"generatedOutputs":paths(&context.classpath.generated_outputs),"capturedAt":context.classpath.captured_at},
            "sourceSets":context.source_sets,"mixinConfigs":context.mixin_configs.iter().map(|c|&c.path).collect::<Vec<_>>(),"accessWideners":context.access_wideners.iter().map(|c|&c.path).collect::<Vec<_>>(),"accessTransformers":context.access_transformers.iter().map(|c|&c.path).collect::<Vec<_>>(),"minecraftJars":paths(&context.minecraft_jars),"indexState":Self::index_state_name(context.index_state)
        })
    }
    pub fn reload_value(
        context: &ProjectContext,
        protocol_version: u32,
        extension_version: &str,
    ) -> serde_json::Value {
        serde_json::json!({"status":"project context reloaded","indexState":Self::index_state_name(context.index_state),"classpathEntries":context.classpath.entry_count(),"lines":Self::format_lines(context,protocol_version,extension_version)})
    }
}

/// Resource contents and host generations invalidate context even when mtimes are unchanged.
pub fn context_fingerprint(root: &Path, facts: Option<&HostProjectFacts>) -> io::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(normalize_path(root).to_string_lossy().as_bytes());
    if let Some(facts) = facts {
        let mut facts = facts.clone();
        facts.classpath.captured_at = 0;
        hasher.update(serde_json::to_vec(&facts).map_err(io::Error::other)?);
    }
    for path in walk_mapping_discovery_regular_files(root)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let resource = !is_under_excluded_directory(relative)
            && (MixinConfigDiscoveryService::is_mixin_config_file(&path)
                || AwAtDiscoveryService::is_access_widener_file(&path)
                || AwAtDiscoveryService::is_access_transformer_file(&path)
                || matches!(
                    name.as_ref(),
                    "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts"
                ));
        let mapping = matches!(
            path.extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
            "tiny" | "srg" | "tsrg"
        );
        if resource || mapping {
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(fs::read(&path)?);
        }
    }
    if facts.is_none() {
        let sets = discover_source_sets(root)?;
        let mut classpath = discover_classpath(root, &sets)?;
        classpath.captured_at = 0;
        hasher.update(serde_json::to_vec(&(sets, classpath)).map_err(io::Error::other)?);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
#[derive(Debug, Clone)]
pub struct CachedProjectSession {
    pub context: Arc<ProjectContext>,
    pub version: u64,
    pub cache_hit: bool,
    pub fingerprint: String,
}
#[derive(Default)]
struct SessionSlot {
    context: Option<Arc<ProjectContext>>,
    fingerprint: String,
    version: u64,
}
type ContextBuilder =
    dyn Fn(&Path, Option<&HostProjectFacts>) -> io::Result<ProjectContext> + Send + Sync;
pub struct FileBasedProjectContextService {
    slots: Mutex<BTreeMap<PathBuf, Arc<Mutex<SessionSlot>>>>,
    builder: Arc<ContextBuilder>,
}
impl Default for FileBasedProjectContextService {
    fn default() -> Self {
        Self::with_builder(build_file_project_context)
    }
}
impl FileBasedProjectContextService {
    pub fn with_builder(
        builder: impl Fn(&Path, Option<&HostProjectFacts>) -> io::Result<ProjectContext>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self {
            slots: Mutex::new(BTreeMap::new()),
            builder: Arc::new(builder),
        }
    }
    fn slot(&self, root: &Path) -> io::Result<Arc<Mutex<SessionSlot>>> {
        Ok(self
            .slots
            .lock()
            .map_err(|_| io::Error::other("project cache lock poisoned"))?
            .entry(root.to_path_buf())
            .or_default()
            .clone())
    }
    pub fn load_cached_session(
        &self,
        workspace_uri: &str,
        facts: Option<&HostProjectFacts>,
    ) -> io::Result<CachedProjectSession> {
        self.load(workspace_uri, facts, false)
    }
    pub fn load_session(
        &self,
        workspace_uri: &str,
        facts: Option<&HostProjectFacts>,
    ) -> io::Result<Arc<ProjectContext>> {
        Ok(self.load_cached_session(workspace_uri, facts)?.context)
    }
    pub fn reindex(
        &self,
        workspace_uri: &str,
        facts: Option<&HostProjectFacts>,
    ) -> io::Result<CachedProjectSession> {
        self.load(workspace_uri, facts, true)
    }
    pub fn reload(
        &self,
        workspace_uri: &str,
        facts: Option<&HostProjectFacts>,
    ) -> io::Result<CachedProjectSession> {
        self.reindex(workspace_uri, facts)
    }
    fn load(
        &self,
        uri: &str,
        facts: Option<&HostProjectFacts>,
        force: bool,
    ) -> io::Result<CachedProjectSession> {
        let root = normalize_path(&uri_to_path(uri).map_err(io::Error::other)?);
        let slot = self.slot(&root)?;
        let mut entry = slot
            .lock()
            .map_err(|_| io::Error::other("project cache lock poisoned"))?;
        let fingerprint = context_fingerprint(&root, facts)?;
        if !force && entry.fingerprint == fingerprint {
            if let Some(context) = &entry.context {
                return Ok(CachedProjectSession {
                    context: context.clone(),
                    version: entry.version,
                    cache_hit: true,
                    fingerprint,
                });
            }
        }
        let context = Arc::new((self.builder)(&root, facts)?);
        entry.version += 1;
        entry.context = Some(context.clone());
        entry.fingerprint = fingerprint.clone();
        Ok(CachedProjectSession {
            context,
            version: entry.version,
            cache_hit: false,
            fingerprint,
        })
    }
    pub fn clear_cache(&self) -> io::Result<()> {
        let slots = self
            .slots
            .lock()
            .map_err(|_| io::Error::other("project cache lock poisoned"))?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for slot in slots {
            slot.lock()
                .map_err(|_| io::Error::other("project cache lock poisoned"))?
                .context = None;
        }
        Ok(())
    }
}
