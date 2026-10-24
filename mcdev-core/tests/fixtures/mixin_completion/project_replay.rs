//! Native replay of every captured project API result against an isolated filesystem.
use mcdev_core::{
    mapping::{self, MappingLookupResult},
    project::*,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mcdev-project-replay-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let p = self.0.canonicalize().unwrap();
        let base = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(p.parent(), Some(base.as_path()));
        assert!(p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("mcdev-project-replay-"));
        fs::remove_dir_all(p).unwrap();
    }
}
fn parse<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("decode {e}: {v}"))
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap()
}
fn encoded<T: Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap()
}
fn camel(s: &str) -> String {
    let mut upper = false;
    let mut out = String::new();
    for c in s.chars() {
        if c == '_' {
            upper = true
        } else {
            out.push(if upper { c.to_ascii_uppercase() } else { c });
            upper = false
        }
    }
    out
}
fn upper_enum(s: &str) -> String {
    if s.chars().all(|c| !c.is_ascii_lowercase()) {
        return s.into();
    }
    let mut out = String::new();
    for (cn, c) in s.chars().enumerate() {
        if c.is_uppercase() && cn > 0 {
            out.push('_')
        }
        out.push(c.to_ascii_uppercase())
    }
    out
}
fn pascal(s: &str) -> String {
    s.split('_')
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|h| h.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect()
}
fn decode_model(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if ["directive", "kind", "modifier"].contains(&k.as_str()) {
                            json!(pascal(v.as_str().unwrap()))
                        } else {
                            decode_model(v)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(decode_model).collect()),
        _ => v.clone(),
    }
}
fn canonical(v: Value, root: &Path, key: &str) -> Value {
    match v {
        Value::Object(mut m) => {
            m.remove("resolver");
            m.remove("resourceIssues");
            m.remove("allEntries");
            m.remove("entryCount");
            Value::Object(
                m.into_iter()
                    .map(|(k, v)| {
                        let k = camel(
                            &k.replace(&root.to_string_lossy().to_string(), "$ROOT")
                                .replace('\\', "/"),
                        );
                        let value = if k == "capturedAt" {
                            json!(0)
                        } else {
                            canonical(v, root, &k)
                        };
                        (k, value)
                    })
                    .collect(),
            )
        }
        Value::Array(a) => {
            let mut a = a
                .into_iter()
                .map(|v| canonical(v, root, key))
                .collect::<Vec<_>>();
            if key == "availableNamespaces" {
                a.sort_by_key(Value::to_string)
            }
            Value::Array(a)
        }
        Value::String(s) => {
            let s = s.replace(&root.to_string_lossy().to_string(), "$ROOT");
            json!(
                if ["directive", "kind", "modifier"].contains(&key) && !s.contains('_') {
                    upper_enum(&s)
                } else if s.contains("$ROOT") {
                    s.replace('\\', "/")
                } else {
                    s
                }
            )
        }
        _ => v,
    }
}
fn materialize(v: &Value, root: &Path) -> Value {
    match v {
        Value::String(s) => json!(s.replace("$ROOT", &root.to_string_lossy())),
        Value::Array(a) => Value::Array(a.iter().map(|v| materialize(v, root)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    (
                        k.replace("$ROOT", &root.to_string_lossy()),
                        materialize(v, root),
                    )
                })
                .collect(),
        ),
        _ => v.clone(),
    }
}
fn base64(s: &str) -> Vec<u8> {
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = vec![];
    for c in s.bytes().filter(|c| *c != b'=') {
        let n = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid base64"),
        };
        bits = (bits << 6) | u32::from(n);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1
        }
    }
    out
}
fn snapshot(args: &Value, root: &Path) {
    for node in args["__filesystem"].as_array().unwrap() {
        let p = root.join(text(node, "path").replace('\\', "/"));
        assert!(
            p.starts_with(root)
                && !p
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
        );
        if node["directory"] == true {
            fs::create_dir_all(p).unwrap()
        } else {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, base64(text(node, "bytes"))).unwrap()
        }
    }
}
fn input(v: &Value) -> ProjectContextInput {
    let mut x = ProjectContextInput::new(text(v, "projectId"), text(v, "root"));
    x.platform = parse(&v["platform"]);
    x.gradle_contents = parse(&v["gradleContents"]);
    x.classpath = parse(&v["classpath"]);
    x.mappings = parse(&v["mappings"]);
    x.mixin_configs = parse(&v["mixinConfigs"]);
    x.access_wideners = parse(&decode_model(&v["accessWideners"]));
    x.access_transformers = parse(&decode_model(&v["accessTransformers"]));
    x.minecraft_jars = parse(&v["minecraftJars"]);
    x.source_sets = parse(&v["sourceSets"]);
    x.index_state = parse(&v["indexState"]);
    x
}
fn context(v: &Value) -> ProjectContext {
    let mut v = decode_model(v);
    v["resourceIssues"] = json!([]);
    parse(&v)
}
fn replay(row: &Value) -> Result<Value, String> {
    let a = &row["args"];
    let path = |k| PathBuf::from(text(a, k));
    let io = |e: std::io::Error| e.to_string();
    Ok(match text(row, "api") {
        "PlatformDetector.detect" => encoded(PlatformDetector::detect(&parse::<Vec<String>>(
            &a["gradleContents"],
        ))),
        "PlatformDetector.detectFromRoot" => {
            encoded(PlatformDetector::detect_from_root(&path("root")).map_err(io)?)
        }
        "ProjectPathFilters.isExcludedDirectorySegment" => {
            json!(["build", ".gradle", "node_modules"]
                .contains(&text(a, "name").to_ascii_lowercase().as_str()))
        }
        "ProjectPathFilters.isUnderExcludedDirectory" => {
            json!(is_under_excluded_directory(&path("path")))
        }
        "ProjectPathFilters.isExcludedFromMappingDiscovery" => {
            json!(is_excluded_from_mapping_discovery(&path("relativePath")))
        }
        "ProjectPathFilters.shouldPruneMappingDiscoveryDirectory" => json!(
            should_prune_mapping_discovery_directory(&path("relativePath"))
        ),
        "ProjectTreeWalker.walkRegularFiles" => {
            encoded(walk_regular_files(&path("root")).map_err(io)?)
        }
        "ProjectTreeWalker.walkMappingDiscoveryRegularFiles" => {
            encoded(walk_mapping_discovery_regular_files(&path("root")).map_err(io)?)
        }
        "AwAtDiscoveryService.isAccessWidenerFile" => {
            json!(AwAtDiscoveryService::is_access_widener_file(&path("path")))
        }
        "AwAtDiscoveryService.isAccessTransformerFile" => json!(
            AwAtDiscoveryService::is_access_transformer_file(&path("path"))
        ),
        "AwAtDiscoveryService.parseAccessWidener" => {
            encoded(AwAtDiscoveryService::parse_access_widener(&path("path")).map_err(io)?)
        }
        "AwAtDiscoveryService.parseAccessTransformer" => {
            encoded(AwAtDiscoveryService::parse_access_transformer(&path("path")).map_err(io)?)
        }
        "AwAtDiscoveryService.discoverAccessWideners" => {
            encoded(AwAtDiscoveryService::discover_access_wideners(&path("root")).map_err(io)?)
        }
        "AwAtDiscoveryService.discoverAccessTransformers" => {
            encoded(AwAtDiscoveryService::discover_access_transformers(&path("root")).map_err(io)?)
        }
        "MappingDiscoveryService.parseMappingFile" => {
            match MappingDiscoveryService::parse_mapping_file(&path("path")) {
                Ok(set) => json!({"mappings":set}),
                Err(e) => return Err(e.to_string()),
            }
        }
        "MappingDiscoveryService.discoverMappingFiles" => {
            encoded(MappingDiscoveryService::discover_mapping_files(&path("root")).map_err(io)?)
        }
        "MappingDiscoveryService.discoverMappingContext" => encoded(
            MappingDiscoveryService::discover_mapping_context(&path("root"), parse(&a["platform"]))
                .map_err(io)?,
        ),
        "MappingDiscoveryService.buildMappingContext" => encoded(
            MappingDiscoveryService::build_mapping_context(
                &parse::<Vec<PathBuf>>(&a["files"]),
                parse(&a["platform"]),
            )
            .map_err(io)?,
        ),
        "MappingResolver.namespaces" => encoded(
            MappingDiscoveryService::build_mapping_context(
                &parse::<Vec<PathBuf>>(&a["files"]),
                parse(&a["platform"]),
            )
            .map_err(io)?
            .resolver
            .namespaces(),
        ),
        "MappingResolver.remapClass" => {
            let c = MappingDiscoveryService::build_mapping_context(
                &parse::<Vec<PathBuf>>(&a["files"]),
                parse(&a["platform"]),
            )
            .map_err(io)?;
            let r = &a["ref"];
            match c.resolver.remap_class(
                &mapping::ClassRef {
                    internal_name: text(r, "internalName").into(),
                    namespace: parse(&r["namespace"]),
                },
                parse(&a["to"]),
            ) {
                MappingLookupResult::Found(v) => {
                    json!({"value":{"internalName":v.internal_name,"namespace":v.namespace}})
                }
                other => panic!("unhandled lookup {other:?}"),
            }
        }
        "MixinConfigDiscoveryService.isMixinConfigFile" => json!(
            MixinConfigDiscoveryService::is_mixin_config_file(&path("path"))
        ),
        "MixinConfigDiscoveryService.parse" => {
            encoded(MixinConfigDiscoveryService::parse(&path("path")))
        }
        "MixinConfigDiscoveryService.parseContent" => encoded(
            MixinConfigDiscoveryService::parse_content(&path("path"), text(a, "content")),
        ),
        "MixinConfigDiscoveryService.normalizeJsonContent" => json!(
            MixinConfigDiscoveryService::normalize_json_content(text(a, "content"))
        ),
        "MixinConfigDiscoveryService.discover" => {
            encoded(MixinConfigDiscoveryService::discover(&path("root")).map_err(io)?)
        }
        "MixinConfigDiscoveryService.sharedConfigs" => {
            encoded(MixinConfigDiscoveryService::shared_configs(
                &parse::<Vec<MixinConfigRef>>(&a["configs"]),
                &parse::<Vec<SourceSetContext>>(&a["allSourceSets"]),
                &path("projectRoot"),
            ))
        }
        "MixinConfigDiscoveryService.configsForSourceSet" => {
            encoded(MixinConfigDiscoveryService::configs_for_source_set(
                &parse::<Vec<MixinConfigRef>>(&a["configs"]),
                &parse::<SourceSetContext>(&a["sourceSet"]),
                &parse::<Vec<SourceSetContext>>(&a["allSourceSets"]),
                &path("projectRoot"),
            ))
        }
        "MixinConfigDiscoveryService.selectForMixin" => {
            encoded(MixinConfigDiscoveryService::select_for_mixin(
                &parse::<Vec<MixinConfigRef>>(&a["configs"]),
                a["mixinClassName"].as_str(),
                a["mixinPackage"].as_str(),
            ))
        }
        "SourceSetResolver.containingSourceSet" => {
            encoded(SourceSetResolver::containing_source_set(
                &parse::<Vec<SourceSetContext>>(&a["sourceSets"]),
                &path("documentPath"),
            ))
        }
        "ProjectContextBuilder.build" => {
            encoded(ProjectContextBuilder::build(input(&a["input"])).map_err(io)?)
        }
        "ProjectContextBuilder.empty" => encoded(ProjectContextBuilder::empty(
            text(a, "projectId"),
            path("root"),
        )),
        "InfoService.formatLines" => json!(InfoService::format_lines(
            &context(&a["context"]),
            a["protocolVersion"].as_u64().unwrap() as u32,
            text(a, "extensionVersion")
        )),
        "InfoService.format" => json!(InfoService::format(
            &context(&a["context"]),
            a["protocolVersion"].as_u64().unwrap() as u32,
            text(a, "extensionVersion")
        )),
        api => panic!("unknown project API {api}"),
    })
}
fn expected(row: &Value) -> Value {
    let mut value = row["output"].clone();
    // Capture used WindowsPath.sorted(). On Unix, preserve the original native
    // Path.sorted() assertion, adapting only APIs whose contract is a sorted
    // path list. No semantic array or source/config text is reordered.
    if !cfg!(windows)
        && [
            "ProjectTreeWalker.walkRegularFiles",
            "ProjectTreeWalker.walkMappingDiscoveryRegularFiles",
            "MappingDiscoveryService.discoverMappingFiles",
        ]
        .contains(&text(row, "api"))
    {
        value
            .as_array_mut()
            .unwrap()
            .sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
    }
    value
}
pub fn replay_case(case: &str) {
    let rows = include_str!("project_legacy_calls.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|r| r["legacyCase"] == case)
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "uncaptured {case}");
    for (i, row) in rows.iter().enumerate() {
        let root = Temp::new();
        snapshot(&row["args"], &root.0);
        let hydrated = materialize(row, &root.0);
        let result = replay(&hydrated);
        if row.get("throwable").is_some() {
            assert!(
                result.is_err(),
                "{case}#{i} {} expected exception",
                row["api"]
            )
        } else {
            let actual = result.unwrap_or_else(|e| panic!("{case}#{i} {} {e}", row["api"]));
            assert_eq!(
                canonical(actual, &root.0, ""),
                canonical(expected(row), &root.0, ""),
                "{case}#{i} {}",
                row["api"]
            );
        }
    }
}
