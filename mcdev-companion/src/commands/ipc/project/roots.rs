//! Read-only workspace files for the original Minecraft discovery domains.
use super::*;
use crate::host_facts::inputs::{RootFileContent, RootFileFilter, RootFileInventory};
use mcdev_core::{
    at::{AccessTransformerParser, AtContextExtractor, AtSlot},
    aw::{AccessWidenerParser, AwContextExtractor, AwSyntaxSlot},
    awat::{AwAtBufferSupport, AwAtFileType},
    mixin_actions::MixinCodeActionService,
    mixin_source::{AnnotationContextExtractor, AnnotationSlot, MixinAnnotation},
    MappingNamespace,
};
use mcdev_protocol::DecodedCommand;

#[derive(Clone, Copy)]
pub(crate) struct DiscoveryNeeds {
    resources: bool,
    mappings: bool,
    references: bool,
}
impl DiscoveryNeeds {
    pub const ALL: Self = Self {
        resources: true,
        mappings: true,
        references: true,
    };
    const NONE: Self = Self {
        resources: false,
        mappings: false,
        references: false,
    };
    const RESOURCES: Self = Self {
        resources: true,
        mappings: false,
        references: false,
    };
    const MAPPINGS: Self = Self {
        resources: false,
        mappings: true,
        references: false,
    };
    const CONTEXT: Self = Self {
        resources: true,
        mappings: true,
        references: false,
    };

    pub fn for_command(command: &DecodedCommand) -> Self {
        let context = command.context();
        let offset = usize::try_from(context.position.line)
            .ok()
            .zip(usize::try_from(context.position.character).ok())
            .and_then(|(line, character)| {
                AwContextExtractor::to_offset(&context.buffer_text, line, character)
            });
        let site = offset.and_then(|offset| {
            AnnotationContextExtractor::extract_at_offset(&context.buffer_text, offset)
        });
        let file_type =
            AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri);
        match command {
            DecodedCommand::Info(_)
            | DecodedCommand::DumpContext(_)
            | DecodedCommand::Reindex(_)
            | DecodedCommand::ReloadProjectContext(_) => Self::CONTEXT,
            DecodedCommand::References(_) => {
                let has_target = match file_type {
                    Some(AwAtFileType::AccessWidener) => offset
                        .and_then(|o| {
                            AwContextExtractor::extract_at_offset(&context.buffer_text, o)
                        })
                        .is_some_and(|c| {
                            matches!(
                                c.slot,
                                AwSyntaxSlot::Owner | AwSyntaxSlot::Name | AwSyntaxSlot::Descriptor
                            )
                        }),
                    Some(AwAtFileType::AccessTransformer) => offset
                        .and_then(|o| {
                            AtContextExtractor::extract_at_offset(&context.buffer_text, o)
                        })
                        .is_some_and(|c| {
                            matches!(
                                c.slot,
                                AtSlot::Owner | AtSlot::MemberName | AtSlot::MemberDescriptor
                            )
                        }),
                    // Member navigation can use normalized declarations outside an annotation.
                    None => true,
                };
                if has_target {
                    Self::ALL
                } else {
                    Self::NONE
                }
            }
            DecodedCommand::Completion(_) => match file_type {
                Some(AwAtFileType::AccessWidener) => {
                    let mapping = offset
                        .and_then(|o| {
                            AwContextExtractor::extract_at_offset(&context.buffer_text, o)
                        })
                        .is_some_and(|c| {
                            matches!(c.slot, AwSyntaxSlot::Name | AwSyntaxSlot::Descriptor)
                                || (c.slot == AwSyntaxSlot::Owner
                                    && c.file_namespace != Some(MappingNamespace::Named))
                        });
                    if mapping {
                        Self::MAPPINGS
                    } else {
                        Self::NONE
                    }
                }
                Some(AwAtFileType::AccessTransformer) => {
                    let mapping = offset
                        .and_then(|o| {
                            AtContextExtractor::extract_at_offset(&context.buffer_text, o)
                        })
                        .is_some_and(|c| {
                            matches!(c.slot, AtSlot::MemberName | AtSlot::MemberDescriptor)
                        });
                    if mapping {
                        Self::MAPPINGS
                    } else {
                        Self::NONE
                    }
                }
                None => {
                    if site.is_some_and(|c| {
                        c.annotation == MixinAnnotation::Share
                            && c.slot == AnnotationSlot::Value
                            && c.attribute_name.as_deref() != Some("namespace")
                    }) {
                        Self::RESOURCES
                    } else {
                        Self::NONE
                    }
                }
            },
            DecodedCommand::Definition(_) | DecodedCommand::Hover(_) => match file_type {
                Some(AwAtFileType::AccessWidener) => {
                    if offset
                        .and_then(|o| {
                            AwContextExtractor::extract_at_offset(&context.buffer_text, o)
                        })
                        .is_some_and(|c| {
                            matches!(
                                c.slot,
                                AwSyntaxSlot::Owner | AwSyntaxSlot::Name | AwSyntaxSlot::Descriptor
                            )
                        })
                    {
                        Self::MAPPINGS
                    } else {
                        Self::NONE
                    }
                }
                Some(AwAtFileType::AccessTransformer) => {
                    if offset
                        .and_then(|o| {
                            AtContextExtractor::extract_at_offset(&context.buffer_text, o)
                        })
                        .is_some_and(|c| {
                            matches!(c.slot, AtSlot::MemberName | AtSlot::MemberDescriptor)
                        })
                    {
                        Self::MAPPINGS
                    } else {
                        Self::NONE
                    }
                }
                None => Self::NONE,
            },
            DecodedCommand::Diagnostics(_)
            | DecodedCommand::Context(_)
            | DecodedCommand::CodeAction(_) => match file_type {
                Some(AwAtFileType::AccessWidener) => {
                    if AccessWidenerParser::parse(&context.buffer_text)
                        .is_ok_and(|file| !file.entries.is_empty())
                    {
                        Self::MAPPINGS
                    } else {
                        Self::NONE
                    }
                }
                Some(AwAtFileType::AccessTransformer) => {
                    if AccessTransformerParser::parse(&context.buffer_text)
                        .is_ok_and(|file| file.entries.iter().any(|entry| entry.name.is_some()))
                    {
                        Self::MAPPINGS
                    } else {
                        Self::NONE
                    }
                }
                None => {
                    if matches!(command,DecodedCommand::CodeAction(request) if !request.diagnostic_codes.is_empty()
                    &&request.diagnostic_codes.iter().all(|code|MixinCodeActionService::is_config_independent_diagnostic(code)))
                    {
                        Self::NONE
                    } else {
                        Self::RESOURCES
                    }
                }
            },
        }
    }
}

#[derive(Default)]
pub(super) struct Domain {
    pub complete: bool,
    pub inventory_complete: bool,
    pub listed: BTreeSet<PathBuf>,
    pub bodies: BTreeMap<PathBuf, RootFileContent>,
    pub missing: BTreeSet<PathBuf>,
}

#[derive(Default)]
pub(super) struct RootDiscovery {
    pub workspace: Option<PathBuf>,
    pub resources: Domain,
    pub mappings: Domain,
    pub references: Domain,
}

fn filter(basenames: &[&str], extensions: &[&str], excluded: &[&str]) -> RootFileFilter {
    RootFileFilter {
        basenames: basenames.iter().map(|value| (*value).into()).collect(),
        extensions: extensions.iter().map(|value| (*value).into()).collect(),
        excluded_directories: excluded.iter().map(|value| (*value).into()).collect(),
    }
}

pub(super) fn resource_filter() -> RootFileFilter {
    filter(
        &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
        &["json", "json5", "aw", "accesswidener", "cfg", "at"],
        &["build", ".gradle", "node_modules"],
    )
}

pub(super) fn mapping_filter() -> RootFileFilter {
    filter(&[], &["tiny", "srg", "tsrg"], &["node_modules"])
}

pub(super) fn reference_filter() -> RootFileFilter {
    filter(&[], &["java", "aw", "accesswidener", "cfg", "at"], &[])
}

pub(super) fn is_gradle(relative: &Path) -> bool {
    if relative.components().count() != 1 {
        return false;
    }
    let name = relative.to_string_lossy();
    [
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ]
    .iter()
    .any(|candidate| {
        if cfg!(windows) {
            name.eq_ignore_ascii_case(candidate)
        } else {
            name == *candidate
        }
    })
}

pub(super) fn is_resource(relative: &Path) -> bool {
    !is_under_excluded_directory(relative.parent().unwrap_or(Path::new("")))
        && (is_gradle(relative)
            || MixinConfigDiscoveryService::is_mixin_config_file(relative)
            || AwAtDiscoveryService::is_access_widener_file(relative)
            || AwAtDiscoveryService::is_access_transformer_file(relative))
}

pub(super) fn is_mapping(relative: &Path) -> bool {
    matches!(
        relative
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("tiny" | "srg" | "tsrg")
    ) && !is_excluded_from_mapping_discovery(relative)
}

pub(super) fn is_reference(path: &Path, sets: &[SourceSetContext]) -> bool {
    let java = path.extension().and_then(|value| value.to_str()) == Some("java");
    let awat = mcdev_core::awat::AwAtBufferSupport::detect_file_type(
        "",
        &path.file_name().unwrap_or_default().to_string_lossy(),
    )
    .is_some();
    sets.iter().any(|set| {
        (java
            && set
                .source_directories
                .iter()
                .any(|root| workspace_relative(root, path).is_some()))
            || (awat
                && set
                    .resource_directories
                    .iter()
                    .any(|root| workspace_relative(root, path).is_some()))
    })
}

fn inventory<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    filter: &RootFileFilter,
) -> Result<Option<RootFileInventory>, FactsError> {
    match inputs.root_file_inventory(filter) {
        Ok(value) => Ok(Some(value)),
        Err(FactsError::Unsupported(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_domain<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    requested_workspace: &str,
    inventory: RootFileInventory,
    required: impl Fn(&Path, &Path) -> bool,
) -> Result<(Option<PathBuf>, Domain), FactsError> {
    let Some(root) = &inventory.root else {
        return Ok((None, Domain::default()));
    };
    if !super::super::same_path(&root.uri, requested_workspace)? {
        return Err(FactsError::Protocol(
            "root-file workspace differs from the command workspace".into(),
        ));
    }
    let workspace = absolute_path(
        &uri_to_path(&root.uri)
            .map_err(FactsError::Protocol)?
            .to_string_lossy(),
    )?;
    let complete =
        matches!(inventory.coverage, Fact::Found { value: true }) && inventory.problems.is_empty();
    let mut result = Domain {
        complete,
        inventory_complete: complete,
        ..Default::default()
    };
    let mut entries = vec![];
    for entry in inventory.entries {
        let path = absolute_path(
            &uri_to_path(&entry.descriptor.uri)
                .map_err(FactsError::Protocol)?
                .to_string_lossy(),
        )?;
        let relative = workspace_relative(&workspace, &path)
            .ok_or_else(|| FactsError::Protocol("root-file URI escaped workspace".into()))?;
        result.listed.insert(path_key(&path));
        if required(&path, &relative) {
            entries.push(entry);
        }
    }
    let bodies = match if entries.is_empty() {
        Ok(vec![])
    } else {
        inputs.root_file_snapshots(&entries)
    } {
        Ok(values) => values,
        Err(FactsError::Unsupported(_)) => {
            result.complete = false;
            for entry in &entries {
                result.missing.insert(path_key(
                    &uri_to_path(&entry.descriptor.uri).map_err(FactsError::Protocol)?,
                ));
            }
            return Ok((Some(workspace), result));
        }
        Err(error) => return Err(error),
    };
    if bodies.len() != entries.len() {
        return Err(FactsError::Protocol(
            "root snapshot cardinality mismatch".into(),
        ));
    }
    for (entry, body) in entries.iter().zip(bodies) {
        match body {
            Fact::Found { value: body } => {
                let path = uri_to_path(&body.uri).map_err(FactsError::Protocol)?;
                if let Some(previous) = result.bodies.insert(path_key(&path), body.clone()) {
                    if previous.text != body.text
                        || previous.identity.sha256 != body.identity.sha256
                        || previous
                            .document_version
                            .zip(body.document_version)
                            .is_some_and(|(a, b)| a != b)
                    {
                        return Err(FactsError::Stale(
                            "conflicting root snapshots for one URI".into(),
                        ));
                    }
                }
            }
            _ => {
                result.complete = false;
                result.missing.insert(path_key(
                    &uri_to_path(&entry.descriptor.uri).map_err(FactsError::Protocol)?,
                ));
            }
        }
    }
    Ok((Some(workspace), result))
}

impl RootDiscovery {
    pub fn platform_complete(&self) -> bool {
        self.resources.inventory_complete
            && self.resources.missing.iter().all(|path| {
                self.workspace
                    .as_ref()
                    .and_then(|root| workspace_relative(root, path))
                    .is_some_and(|relative| !is_gradle(&relative))
            })
    }
}

pub(super) fn discover<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    workspace: &str,
    sets: &[SourceSetContext],
    needs: DiscoveryNeeds,
) -> Result<RootDiscovery, FactsError> {
    let mut result = RootDiscovery::default();
    if let Some(inventory) = if needs.resources || needs.mappings {
        inventory(inputs, &resource_filter())?
    } else {
        None
    } {
        let (root, mut domain) = read_domain(inputs, workspace, inventory, |_, relative| {
            is_gradle(relative) || (needs.resources && is_resource(relative))
        })?;
        domain.complete &= needs.resources;
        result.workspace = root;
        result.resources = domain;
    }
    if let Some(inventory) = if needs.mappings {
        inventory(inputs, &mapping_filter())?
    } else {
        None
    } {
        let (root, domain) = read_domain(inputs, workspace, inventory, |_, relative| {
            is_mapping(relative)
        })?;
        result.workspace = result.workspace.or(root);
        result.mappings = domain;
    }
    if let Some(inventory) = if needs.references {
        inventory(inputs, &reference_filter())?
    } else {
        None
    } {
        let (root, mut domain) = read_domain(inputs, workspace, inventory, |path, _| {
            is_reference(path, sets)
        })?;
        if let Some(root) = &root {
            domain.complete &= sets
                .iter()
                .flat_map(|set| {
                    set.source_directories
                        .iter()
                        .chain(&set.resource_directories)
                })
                .all(|path| workspace_relative(root, path).is_some());
        }
        result.workspace = result.workspace.or(root);
        result.references = domain;
    }
    Ok(result)
}
