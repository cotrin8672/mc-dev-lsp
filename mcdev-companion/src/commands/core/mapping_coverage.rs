use super::*;
use mcdev_core::{
    at::{AccessTransformerParser, AtContextExtractor, AtSlot},
    aw::{AwContextExtractor, AwSyntaxSlot},
    awat::AwAtFileType,
    mapping::{
        ClassRef, FieldRef, MappingLookupResult, MappingResolver, MethodRef, ProjectMappingContext,
    },
    MappingNamespace,
};
use std::{borrow::Cow, collections::BTreeSet};

fn unavailable_mapping() -> CommandFailure {
    unavailable(
        McdevErrorCode::IncompleteProjectContext,
        "complete root/cache mapping discovery is unavailable",
    )
}

struct UnavailableMapping;
impl MappingResolver for UnavailableMapping {
    fn namespaces(&self) -> BTreeSet<MappingNamespace> {
        std::panic::panic_any(unavailable_mapping())
    }
    fn remap_class(
        &self,
        reference: &ClassRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<ClassRef> {
        if reference.namespace == to {
            MappingLookupResult::Found(reference.clone())
        } else {
            std::panic::panic_any(unavailable_mapping())
        }
    }
    fn remap_field(
        &self,
        reference: &FieldRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<FieldRef> {
        if reference.namespace == to {
            MappingLookupResult::Found(reference.clone())
        } else {
            std::panic::panic_any(unavailable_mapping())
        }
    }
    fn remap_method(
        &self,
        reference: &MethodRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<MethodRef> {
        if reference.namespace == to {
            MappingLookupResult::Found(reference.clone())
        } else {
            std::panic::panic_any(unavailable_mapping())
        }
    }
    fn remap_descriptor(
        &self,
        descriptor: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> MappingLookupResult<String> {
        if from == to {
            MappingLookupResult::Found(descriptor.into())
        } else {
            std::panic::panic_any(unavailable_mapping())
        }
    }
}

pub(super) fn context(
    mapping: &ProjectMappingContext,
    complete: bool,
) -> Cow<'_, ProjectMappingContext> {
    if complete {
        Cow::Borrowed(mapping)
    } else {
        let mut result = mapping.clone();
        result.resolver = Arc::new(UnavailableMapping);
        Cow::Owned(result)
    }
}

pub(super) fn guard(
    complete: bool,
    file_type: AwAtFileType,
    text: &str,
    position: Option<(usize, usize)>,
) -> Result<(), CommandFailure> {
    if complete {
        return Ok(());
    }
    let needs = match (file_type, position) {
        (AwAtFileType::AccessTransformer, Some((line, character))) => {
            AwContextExtractor::to_offset(text, line, character)
                .and_then(|offset| AtContextExtractor::extract_at_offset(text, offset))
                .is_some_and(|context| {
                    matches!(context.slot, AtSlot::MemberName | AtSlot::MemberDescriptor)
                })
        }
        (AwAtFileType::AccessTransformer, None) => AccessTransformerParser::parse(text)
            .ok()
            .is_some_and(|file| file.entries.iter().any(|entry| entry.name.is_some())),
        (AwAtFileType::AccessWidener, Some((line, character))) => {
            AwContextExtractor::to_offset(text, line, character)
                .and_then(|offset| AwContextExtractor::extract_at_offset(text, offset))
                .is_some_and(|context| {
                    context.file_namespace.is_none()
                        && matches!(
                            context.slot,
                            AwSyntaxSlot::Owner | AwSyntaxSlot::Name | AwSyntaxSlot::Descriptor
                        )
                })
        }
        (AwAtFileType::AccessWidener, None) => false,
    };
    if needs {
        Err(unavailable_mapping())
    } else {
        Ok(())
    }
}
