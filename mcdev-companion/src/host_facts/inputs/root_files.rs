//! Working root-files-v1 draft. These read-only tokens grant no source/resource/Code authority.
use super::*;

const MAX_ROOT_FILES: usize = 16_000;
const MAX_ROOT_BODY: usize = 16 * 1024 * 1024;
const MAX_ROOT_CHUNK: usize = 96 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RootFileFilter {
    #[serde(default)]
    pub basenames: BTreeSet<String>,
    #[serde(default)]
    pub extensions: BTreeSet<String>,
    #[serde(default)]
    pub excluded_directories: BTreeSet<String>,
}

impl RootFileFilter {
    pub fn validate(&self) -> Result<(), FactsError> {
        let literal = |value: &str| {
            !value.is_empty()
                && value.len() <= 256
                && !matches!(value, "." | "..")
                && !value.chars().any(|c| {
                    c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '[' | ']')
                })
        };
        if self.basenames.len() + self.extensions.len() + self.excluded_directories.len() > 128
            || self
                .basenames
                .iter()
                .chain(&self.extensions)
                .chain(&self.excluded_directories)
                .any(|value| !literal(value))
            || self.extensions.iter().any(|value| value.starts_with('.'))
        {
            return Err(protocol("invalid root-file filter"));
        }
        Ok(())
    }

    fn effective(&self) -> Self {
        let lower =
            |values: &BTreeSet<String>| values.iter().map(|value| value.to_lowercase()).collect();
        Self {
            basenames: lower(&self.basenames),
            extensions: lower(&self.extensions),
            excluded_directories: lower(&self.excluded_directories),
        }
    }

    fn includes(&self, relative: &str) -> bool {
        let mut segments = relative.split('/').peekable();
        let mut basename = "";
        while let Some(segment) = segments.next() {
            if segments.peek().is_some()
                && self.excluded_directories.contains(&segment.to_lowercase())
            {
                return false;
            }
            basename = segment;
        }
        let name = basename.to_lowercase();
        (self.basenames.is_empty() && self.extensions.is_empty())
            || self.basenames.contains(&name)
            || std::path::Path::new(&name)
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| self.extensions.contains(extension))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceRootKind {
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFileDomain {
    pub identity: String,
    pub uri: String,
    pub root_kind: WorkspaceRootKind,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFileDescriptor {
    pub file: String,
    pub uri: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootFileEntry {
    pub descriptor: RootFileDescriptor,
    pub root: RootFileDomain,
    pub filter: RootFileFilter,
    scope: u64,
}

#[derive(Debug, Clone)]
pub struct RootFileInventory {
    pub root: Option<RootFileDomain>,
    pub filter: RootFileFilter,
    pub entries: Vec<RootFileEntry>,
    pub coverage: Fact<bool>,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootFileIdentity {
    pub kind: String,
    pub revision: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFileContent {
    pub file: String,
    pub root: RootFileDomain,
    pub uri: String,
    pub relative_path: String,
    pub identity: RootFileIdentity,
    pub document_version: Option<i32>,
    pub size: usize,
    pub text: String,
}

#[derive(Deserialize)]
struct RootPage {
    root: Option<RootFileDomain>,
    filter: RootFileFilter,
    files: Vec<RootFileDescriptor>,
    next: Option<String>,
    coverage: Fact<bool>,
    problems: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootChunk {
    file: String,
    root: RootFileDomain,
    uri: String,
    relative_path: String,
    identity: RootFileIdentity,
    document_version: Option<i32>,
    size: usize,
    offset: usize,
    text: String,
    next: Option<String>,
}

fn decimal_revision(revision: &str) -> bool {
    revision
        .parse::<u64>()
        .ok()
        .is_some_and(|value| value.to_string() == revision)
}

fn uri_key(uri: &str) -> Result<String, FactsError> {
    if !uri.starts_with("file://") {
        return Err(protocol("root file URI must use file scheme"));
    }
    let path = mcdev_core::project::uri_to_path(uri).map_err(protocol)?;
    let key = path.to_string_lossy().replace('\\', "/");
    if key.is_empty() {
        return Err(protocol("root file URI has no path"));
    }
    Ok(if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    })
}

fn validate_domain(root: &RootFileDomain) -> Result<(), FactsError> {
    if !valid_token(&root.identity) || !decimal_revision(&root.revision) {
        return Err(protocol("invalid root-file domain identity or revision"));
    }
    uri_key(&root.uri)?;
    Ok(())
}

fn validate_file(
    file: &RootFileDescriptor,
    root: &RootFileDomain,
    filter: &RootFileFilter,
) -> Result<(), FactsError> {
    let relative = &file.relative_path;
    if !valid_token(&file.file)
        || relative.contains(['\\', '\0'])
        || relative
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || !filter.includes(relative)
    {
        return Err(protocol(
            "root-file descriptor is outside its literal filter or relative domain",
        ));
    }
    let expected = format!(
        "{}/{}",
        uri_key(&root.uri)?.trim_end_matches('/'),
        if cfg!(windows) {
            relative.to_lowercase()
        } else {
            relative.clone()
        }
    );
    if uri_key(&file.uri)? != expected {
        return Err(protocol(
            "root-file URI and relative root membership disagree",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct RootAssembly {
    after: Option<String>,
    seen: BTreeSet<String>,
    first: Option<RootChunk>,
    text: String,
    result: Option<Fact<RootFileContent>>,
}

impl RootAssembly {
    fn push(&mut self, entry: &RootFileEntry, fact: Fact<RootChunk>) -> Result<(), FactsError> {
        validate_fact(&fact)?;
        let chunk = match fact {
            Fact::Found { value } => value,
            Fact::Unknown { reasons } => {
                self.result = Some(Fact::Unknown { reasons });
                return Ok(());
            }
            Fact::DefinitelyAbsent => {
                return Err(protocol("rootFileSnapshot cannot certify absence"))
            }
        };
        if chunk.file != entry.descriptor.file
            || chunk.root != entry.root
            || chunk.uri != entry.descriptor.uri
            || chunk.relative_path != entry.descriptor.relative_path
            || chunk.identity.kind != "rootFile"
            || !decimal_revision(&chunk.identity.revision)
            || !valid_digest(&chunk.identity.sha256)
            || chunk.size > MAX_ROOT_BODY
            || chunk.offset != self.text.len()
            || chunk.text.len() > MAX_ROOT_CHUNK
        {
            return Err(protocol(
                "root-file stream identity, root, offset or byte limit mismatch",
            ));
        }
        if let Some(first) = &self.first {
            if chunk.identity != first.identity
                || chunk.document_version != first.document_version
                || chunk.size != first.size
            {
                return Err(protocol(
                    "root-file source identity, document version or size changed between chunks",
                ));
            }
        }
        let end = self
            .text
            .len()
            .checked_add(chunk.text.len())
            .ok_or_else(|| protocol("root-file size overflow"))?;
        if end > chunk.size
            || (chunk.next.is_some() && (chunk.text.is_empty() || end >= chunk.size))
        {
            return Err(protocol("invalid root-file text continuation"));
        }
        self.text.push_str(&chunk.text);
        self.after = next_cursor(chunk.next.clone(), &mut self.seen)?;
        if self.first.is_none() {
            self.first = Some(chunk);
        }
        if self.after.is_some() {
            return Ok(());
        }
        let first = self
            .first
            .take()
            .expect("completed root-file stream has its first chunk");
        if self.text.len() != first.size
            || !digest_matches(self.text.as_bytes(), &first.identity.sha256)
        {
            return Err(protocol(
                "root-file terminal byte length or full digest mismatch",
            ));
        }
        self.result = Some(Fact::Found {
            value: RootFileContent {
                file: first.file,
                root: first.root,
                uri: first.uri,
                relative_path: first.relative_path,
                identity: first.identity,
                document_version: first.document_version,
                size: first.size,
                text: std::mem::take(&mut self.text),
            },
        });
        Ok(())
    }
}

impl<R: FactReader> ScopedInputs<R> {
    pub fn root_file_inventory(
        &mut self,
        filter: &RootFileFilter,
    ) -> Result<RootFileInventory, FactsError> {
        filter.validate()?;
        let effective = filter.effective();
        let mut entries = Vec::new();
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut issued = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut root: Option<RootFileDomain> = None;
        let mut problems = BTreeSet::new();
        loop {
            let page: RootPage = self.one(
                InputOp::RootFileInventory,
                json!({"op":"rootFileInventory","filter":filter,"after":after}),
            )?;
            validate_fact(&page.coverage)?;
            if page.filter != *filter
                || page.files.len() > MAX_BATCH
                || (page.next.is_some()
                    && (!matches!(page.coverage, Fact::Unknown { .. }) || page.files.is_empty()))
                || matches!(
                    page.coverage,
                    Fact::Found { value: false } | Fact::DefinitelyAbsent
                )
                || (page.root.is_none()
                    && (!page.files.is_empty()
                        || page.next.is_some()
                        || page.coverage.found().is_some()))
            {
                return Err(protocol(
                    "invalid root-file inventory filter, coverage or page progress",
                ));
            }
            if let Some(domain) = &page.root {
                validate_domain(domain)?;
                if root.as_ref().is_some_and(|root| root != domain) {
                    return Err(protocol("root-file inventory domain changed between pages"));
                }
                insert_consistent(
                    &mut self.root_domains,
                    &domain.identity,
                    &(domain.clone(), filter.clone()),
                )?;
            } else if root.is_some() {
                return Err(protocol("root-file inventory lost its root domain"));
            }
            root = page.root;
            problems.extend(page.problems);
            if page.coverage.found() == Some(&true) && !problems.is_empty() {
                return Err(protocol(
                    "root-file inventory claims completeness with unresolved problems",
                ));
            }
            for descriptor in page.files {
                let domain = root
                    .as_ref()
                    .expect("nonempty inventory has a checked root");
                validate_file(&descriptor, domain, &effective)?;
                if !issued.insert(descriptor.file.clone())
                    || !paths.insert(uri_key(&descriptor.uri)?)
                {
                    return Err(protocol("duplicate root-file token or URI in inventory"));
                }
                let entry = RootFileEntry {
                    descriptor,
                    root: domain.clone(),
                    filter: filter.clone(),
                    scope: self.scope,
                };
                insert_consistent(&mut self.root_files, &entry.descriptor.file, &entry)?;
                entries.push(entry);
                if entries.len() > MAX_ROOT_FILES {
                    return Err(protocol("root-file inventory entry limit"));
                }
            }
            after = next_cursor(page.next, &mut seen)?;
            if after.is_none() {
                return Ok(RootFileInventory {
                    root,
                    filter: filter.clone(),
                    entries,
                    coverage: page.coverage,
                    problems: problems.into_iter().collect(),
                });
            }
        }
    }

    pub fn root_file_snapshot(
        &mut self,
        entry: &RootFileEntry,
    ) -> Result<Fact<RootFileContent>, FactsError> {
        Ok(self
            .root_file_snapshots(std::slice::from_ref(entry))?
            .remove(0))
    }

    pub fn root_file_snapshots(
        &mut self,
        entries: &[RootFileEntry],
    ) -> Result<Vec<Fact<RootFileContent>>, FactsError> {
        for entry in entries {
            check_entry(
                entry.scope,
                self.scope,
                &entry.descriptor.file,
                entry,
                &self.root_files,
            )?;
        }
        if entries.is_empty() {
            self.read(InputOp::RootFileSnapshot, &[])?;
            return Ok(Vec::new());
        }
        let mut states: Vec<_> = entries.iter().map(|_| RootAssembly::default()).collect();
        loop {
            let pending: Vec<_> = states
                .iter()
                .enumerate()
                .filter_map(|(index, state)| state.result.is_none().then_some(index))
                .collect();
            if pending.is_empty() {
                return Ok(states
                    .into_iter()
                    .map(|state| state.result.expect("completed root-file snapshot"))
                    .collect());
            }
            let queries: Vec<_> = pending
                .iter()
                .map(|&index| {
                    json!({"op":"rootFileSnapshot",
                "file":entries[index].descriptor.file,"after":states[index].after})
                })
                .collect();
            let values = self.read(InputOp::RootFileSnapshot, &queries)?;
            for (index, value) in pending.into_iter().zip(values) {
                states[index].push(&entries[index], decode(value)?)?;
            }
            let bytes = states
                .iter()
                .try_fold(0usize, |used, state| {
                    used.checked_add(
                        state.text.len()
                            + state
                                .result
                                .as_ref()
                                .and_then(Fact::found)
                                .map_or(0, |content| content.text.len()),
                    )
                })
                .ok_or_else(|| protocol("root-file aggregate size overflow"))?;
            if bytes > MAX_RESOURCE_BYTES {
                return Err(protocol("root-file aggregate text limit"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_file_published_serializer_sample_preserves_overlay_and_domain_identity() {
        let bytes = include_bytes!(
            "../../../tests/fixtures/host_inputs/root_files/actual-serializer-sample.json"
        );
        assert_eq!(bytes.len(), 1634);
        assert!(digest_matches(
            bytes,
            "af49d91e7e42dada7314a953b710ebe2d998176b7ed00ee361b410db1c3a4c41"
        ));
        let sample: Value = serde_json::from_slice(bytes).unwrap();
        let provenance: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/host_inputs/root_files/provenance.json"
        ))
        .unwrap();
        assert_eq!(
            provenance["hostCommit"],
            "392076bc5e364e02426450c574c1227db2c3be8f"
        );
        assert_eq!(
            sample["provenance"]["sha256"],
            provenance["producerProofSha256"]
        );
        assert_eq!(sample["provenance"]["phase"], "fresh");
        assert!(sample.get("coverage").is_none() && sample.get("files").is_none());
        let root: RootFileDomain = decode(sample["inventoryRoot"].clone()).unwrap();
        let descriptor: RootFileDescriptor = decode(sample["descriptor"].clone()).unwrap();
        validate_domain(&root).unwrap();
        let filter = RootFileFilter::default();
        validate_file(&descriptor, &root, &filter.effective()).unwrap();
        // This entry/filter/local scope is unit scaffolding, not a captured inventory page.
        let entry = RootFileEntry {
            descriptor,
            root,
            filter,
            scope: 0,
        };
        let mut assembly = RootAssembly::default();
        assembly
            .push(&entry, decode(sample["snapshot"].clone()).unwrap())
            .unwrap();
        assert!(assembly.after.is_none());
        let content = assembly.result.unwrap().found().unwrap().clone();
        assert_eq!(content.file, "4:9");
        assert_eq!(content.root.identity, "4:6");
        assert_eq!(content.root.revision, "5");
        assert_eq!(content.identity.revision, "7");
        assert_eq!(content.document_version, Some(4));
        assert_eq!(content.size, 49);
        assert_eq!(content.text.len(), 49);
        assert_eq!(
            content.text,
            "{\"scope\":\"java-client\",\"value\":\"overlay-更新\"}\n"
        );
        assert_eq!(
            content.identity.sha256,
            "93c36749ee4a40b8132e8715de31b7aafecfb176d31178fd3f597e5f591382c0"
        );
        assert!(digest_matches(
            content.text.as_bytes(),
            &content.identity.sha256
        ));
    }
}
