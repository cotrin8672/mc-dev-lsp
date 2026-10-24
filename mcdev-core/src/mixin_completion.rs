use crate::mixin_edits::MixinImportEditBuilder;
use crate::mixin_source::{
    annotation_values, annotations, matching, source_tokens, AnnotationContextExtractor,
    SourceTokenKind,
};
use crate::{
    bytecode::{AtTargetCandidate, AtTargetKind, ConstantValue},
    descriptor::{self, MemberTarget},
    mapping::{self, MappingLookupResult, MappingResolver},
    mixin_source::{
        AnnotationContext, AnnotationSlot, JavaTypeDescriptorResolver, MixinAnnotation,
        MixinTargetResolver,
    },
    *,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MixinClassInsertMode {
    #[default]
    Import,
    Fqn,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InjectMethodDescriptorMode {
    #[default]
    Auto,
    Always,
    Never,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinCompletionOptions {
    pub class_insert_mode: MixinClassInsertMode,
    pub inject_method_descriptor_mode: InjectMethodDescriptorMode,
    pub preferred_at_target: String,
}
impl Default for MixinCompletionOptions {
    fn default() -> Self {
        Self {
            class_insert_mode: MixinClassInsertMode::Import,
            inject_method_descriptor_mode: InjectMethodDescriptorMode::Auto,
            preferred_at_target: "smart".into(),
        }
    }
}
pub(crate) fn item(
    label: String,
    insert_text: String,
    kind: McCompletionKind,
    sort_key: String,
    source: &str,
) -> McCompletionItem {
    McCompletionItem {
        label,
        detail: None,
        documentation: None,
        filter_text: insert_text.clone(),
        insert_text,
        kind,
        sort_key,
        metadata: McCompletionMetadata {
            source: source.into(),
            ..Default::default()
        },
        additional_edits: vec![],
        insert_text_format: McCompletionInsertTextFormat::PlainText,
    }
}
pub struct ConstantAtHintFormatter;
impl ConstantAtHintFormatter {
    pub fn hint_for(candidate: &AtTargetCandidate) -> Option<String> {
        if candidate.kind != AtTargetKind::Constant {
            return None;
        }
        Some(match candidate.constant_value.as_ref()? {
            value @ (ConstantValue::StringValue(_) | ConstantValue::StringUtf16Value(_)) => {
                format!("@Constant(stringValue = {})", value.java_string_literal()?)
            }
            ConstantValue::IntValue(n) => format!("@Constant(intValue = {n})"),
            ConstantValue::LongValue(n) => format!("@Constant(longValue = {n}L)"),
            ConstantValue::FloatValue(n) => format!(
                "@Constant(floatValue = {})",
                crate::bytecode::constant_label(Some(&ConstantValue::FloatValue(*n)))
            ),
            ConstantValue::DoubleValue(n) => format!(
                "@Constant(doubleValue = {})",
                crate::bytecode::constant_label(Some(&ConstantValue::DoubleValue(*n)))
            ),
            ConstantValue::ClassLiteral(n) => {
                format!("@Constant(classValue = \"{}.class\")", n.replace('/', "."))
            }
            ConstantValue::NullValue => "@Constant(nullValue = true)".into(),
        })
    }
}

pub struct AtTargetCompletionService;
impl AtTargetCompletionService {
    pub fn format_target(c: &AtTargetCandidate) -> String {
        match c.kind {
            AtTargetKind::Return => "RETURN".into(),
            AtTargetKind::Constant => c.display_label.clone(),
            AtTargetKind::Field => format!("L{};{}:{}", c.owner, c.name, c.descriptor),
            AtTargetKind::New if c.name != "<init>" => format!("L{};", c.owner),
            _ => format!("L{};{}{}", c.owner, c.name, c.descriptor),
        }
    }
    pub fn complete(
        context: &AnnotationContext,
        candidates: &[AtTargetCandidate],
    ) -> Vec<McCompletionItem> {
        if context.annotation != MixinAnnotation::At || context.slot != AnnotationSlot::Target {
            return vec![];
        }
        let partial = context.partial_value.trim_matches('"').to_lowercase();
        let mut seen = BTreeSet::new();
        candidates
            .iter()
            .filter_map(|c| {
                let target = Self::format_target(c);
                if !partial.is_empty()
                    && !c.display_label.to_lowercase().contains(&partial)
                    && !target.to_lowercase().contains(&partial)
                    || !seen.insert(target.clone())
                {
                    return None;
                }
                let mut result = item(
                    c.display_label.clone(),
                    target.clone(),
                    McCompletionKind::Value,
                    format!("0400_{}", c.name),
                    "mixin.atTarget",
                );
                result.detail = Some(c.detail.clone());
                result.documentation = Some(ConstantAtHintFormatter::hint_for(c).unwrap_or(target));
                result.filter_text = format!("{} {} {}", c.name, c.display_label, c.detail);
                result.metadata.owner = Some(c.owner.clone());
                result.metadata.name = Some(c.name.clone());
                result.metadata.descriptor = Some(c.descriptor.clone());
                result.metadata.namespace = Some(format!("{:?}", c.namespace).to_ascii_uppercase());
                Some(result)
            })
            .collect()
    }
}
pub struct AtTargetInsertFormatter;
impl AtTargetInsertFormatter {
    pub fn format_insert(
        candidate: &AtTargetCandidate,
        resolver: Option<&dyn MappingResolver>,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> String {
        let target = AtTargetCompletionService::format_target(candidate);
        let Some(resolver) = resolver else {
            return target;
        };
        if from == to
            || matches!(
                candidate.kind,
                AtTargetKind::Return | AtTargetKind::Constant
            )
        {
            return target;
        }
        match descriptor::parse_member_target(&target) {
            Ok(MemberTarget::Method {
                owner,
                name,
                descriptor,
            }) => match resolver.remap_method(
                &mapping::MethodRef {
                    owner,
                    name,
                    descriptor: descriptor::method_to_descriptor(&descriptor),
                    namespace: from,
                },
                to,
            ) {
                MappingLookupResult::Found(r) => format!("L{};{}{}", r.owner, r.name, r.descriptor),
                _ => target,
            },
            Ok(MemberTarget::Field {
                owner,
                name,
                descriptor,
            }) => match resolver.remap_field(
                &mapping::FieldRef {
                    owner,
                    name,
                    descriptor: descriptor::to_descriptor(&descriptor),
                    namespace: from,
                },
                to,
            ) {
                MappingLookupResult::Found(r) => {
                    format!("L{};{}:{}", r.owner, r.name, r.descriptor)
                }
                _ => target,
            },
            _ => target,
        }
    }
}
const AT_VALUES: &[&str] = &[
    "HEAD",
    "CTOR_HEAD",
    "RETURN",
    "TAIL",
    "INVOKE",
    "INVOKE_STRING",
    "INVOKE_ASSIGN",
    "FIELD",
    "NEW",
    "CONSTANT",
    "LOAD",
    "STORE",
    "MIXINEXTRAS:EXPRESSION",
];
pub struct AtValueCompletionService<'a> {
    specifier_supported: Box<dyn Fn() -> bool + 'a>,
    additional_values: Box<dyn Fn() -> Vec<String> + 'a>,
}
impl Default for AtValueCompletionService<'_> {
    fn default() -> Self {
        Self::new(|| false, Vec::new)
    }
}
impl<'a> AtValueCompletionService<'a> {
    pub fn new(probe: impl Fn() -> bool + 'a, additional: impl Fn() -> Vec<String> + 'a) -> Self {
        Self {
            specifier_supported: Box::new(probe),
            additional_values: Box::new(additional),
        }
    }
    pub fn complete(&self, context: &AnnotationContext) -> Vec<McCompletionItem> {
        if context.annotation != MixinAnnotation::At || context.slot != AnnotationSlot::Value {
            return vec![];
        }
        use MixinAnnotation::*;
        let base: &[&str] = if context.at_inside_slice {
            AT_VALUES
        } else {
            match context.parent_injector_annotation {
                Some(ModifyReturnValue) => &["RETURN", "TAIL", "MIXINEXTRAS:EXPRESSION"],
                Some(ModifyExpressionValue) => &[
                    "INVOKE",
                    "FIELD",
                    "NEW",
                    "CONSTANT",
                    "MIXINEXTRAS:EXPRESSION",
                ],
                Some(ModifyReceiver | WrapWithCondition) => {
                    &["INVOKE", "FIELD", "MIXINEXTRAS:EXPRESSION"]
                }
                Some(WrapOperation) => &["INVOKE", "FIELD", "NEW", "MIXINEXTRAS:EXPRESSION"],
                _ => AT_VALUES,
            }
        };
        let mut values = base.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut seen = values
            .iter()
            .map(|s| s.to_ascii_uppercase())
            .collect::<BTreeSet<_>>();
        for value in (self.additional_values)() {
            let upper = value.to_ascii_uppercase();
            let built = AT_VALUES
                .iter()
                .filter(|b| upper == **b || upper.starts_with(&format!("{b}:")))
                .max_by_key(|b| b.len());
            if built.is_some_and(|b| !base.contains(b)) {
                continue;
            }
            if seen.insert(upper) {
                values.push(value);
            }
        }
        let partial = context.partial_value.trim_matches('"').to_ascii_uppercase();
        let matched = values
            .iter()
            .filter(|v| {
                partial.len() > v.len()
                    && partial.starts_with(&format!("{}:", v.to_ascii_uppercase()))
            })
            .max_by_key(|v| v.len());
        let candidates = if let Some(code) =
            matched.filter(|_| context.at_inside_slice || (self.specifier_supported)())
        {
            let prefix = &partial[code.len() + 1..];
            ["FIRST", "LAST", "ONE", "ALL", "DEFAULT"]
                .into_iter()
                .filter(|s| s.starts_with(prefix))
                .map(|s| format!("{code}:{s}"))
                .collect()
        } else {
            values
                .into_iter()
                .filter(|v| v.to_ascii_uppercase().starts_with(&partial))
                .collect::<Vec<_>>()
        };
        candidates
            .into_iter()
            .map(|v| {
                let mut result = item(
                    v.clone(),
                    v.clone(),
                    McCompletionKind::Value,
                    format!("0300_{v}"),
                    "mixin.atValue",
                );
                result.detail = Some("@At injection point".into());
                result.metadata.name = Some(v);
                result
            })
            .collect()
    }
}

pub struct InjectMethodCompletionService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> InjectMethodCompletionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn complete(
        &self,
        context: &AnnotationContext,
        options: &MixinCompletionOptions,
    ) -> Vec<McCompletionItem> {
        if context.slot != AnnotationSlot::Method {
            return vec![];
        }
        let owners = MixinTargetResolver::resolve_targets(
            &context.mixin_target_internal_names,
            self.class_index,
            None,
        );
        let partial = context.partial_value.trim_matches('"');
        let prefix = partial.split('(').next().unwrap_or(partial);
        let mut result = vec![];
        for owner in owners {
            let methods = self
                .class_index
                .get_methods(&owner)
                .into_iter()
                .filter(|m| m.name.starts_with(prefix))
                .collect::<Vec<_>>();
            let mut names = vec![];
            for m in &methods {
                if !names.contains(&m.name) {
                    names.push(m.name.clone());
                }
            }
            for name in names {
                let overloads = methods
                    .iter()
                    .filter(|m| m.name == name)
                    .collect::<Vec<_>>();
                let qualified = options.inject_method_descriptor_mode
                    == InjectMethodDescriptorMode::Always
                    || (options.inject_method_descriptor_mode == InjectMethodDescriptorMode::Auto
                        && overloads.len() > 1);
                let count = if qualified { overloads.len() } else { 1 };
                for method in overloads.into_iter().take(count) {
                    let insert = if qualified {
                        format!("{}{}", method.name, method.descriptor)
                    } else {
                        method.name.clone()
                    };
                    let mut value = item(
                        method.readable_signature.clone(),
                        insert,
                        McCompletionKind::Value,
                        format!("0200_{}", method.name),
                        "mixin.injectMethod",
                    );
                    value.detail = Some(owner.replace('/', ".").replace('$', "."));
                    value.documentation = Some(method.descriptor.clone());
                    value.filter_text = format!("{} {}", method.name, method.readable_signature);
                    value.metadata.owner = Some(owner.clone());
                    value.metadata.name = Some(method.name.clone());
                    value.metadata.descriptor = Some(method.descriptor.clone());
                    value.metadata.namespace = Some("NAMED".into());
                    result.push(value);
                }
            }
        }
        result.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
        result
    }
}
pub struct MixinTargetCompletionService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> MixinTargetCompletionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn complete(
        &self,
        context: &AnnotationContext,
        options: &MixinCompletionOptions,
        source: &str,
    ) -> Vec<McCompletionItem> {
        if context.annotation != MixinAnnotation::Mixin {
            return vec![];
        }
        let (classes, strings) = match context.slot {
            AnnotationSlot::Class => {
                let classes = self
                    .class_index
                    .find_classes(&context.partial_value, CLASS_COMPLETION_LIMIT);
                if context.is_kotlin && !source.is_empty() {
                    let imports = JavaTypeDescriptorResolver::imports_for(source);
                    let mut aliases = imports
                        .explicit
                        .iter()
                        .filter(|(name, _)| {
                            name.to_lowercase()
                                .starts_with(&context.partial_value.to_lowercase())
                        })
                        .filter_map(|(_, fqn)| self.class_index.find_class_by_fqn(fqn))
                        .collect::<Vec<_>>();
                    aliases.extend(classes);
                    let mut seen = BTreeSet::new();
                    (
                        aliases
                            .into_iter()
                            .filter(|c| seen.insert(c.internal_name.clone()))
                            .take(CLASS_COMPLETION_LIMIT)
                            .collect(),
                        false,
                    )
                } else {
                    (classes, false)
                }
            }
            AnnotationSlot::Targets => {
                let partial = context.partial_value.trim_matches('"');
                let prefix = partial.rsplit(['.', '/']).next().unwrap_or(partial);
                (
                    self.class_index
                        .find_classes(prefix, CLASS_COMPLETION_LIMIT),
                    true,
                )
            }
            _ => return vec![],
        };
        classes
            .into_iter()
            .map(|entry| {
                let reference = if context.is_kotlin && !source.is_empty() {
                    MixinImportEditBuilder::reference_for_internal_name(
                        source,
                        &entry.internal_name,
                        &BTreeSet::new(),
                    )
                    .text
                } else {
                    entry.simple_name.clone()
                };
                let insert = if strings {
                    entry.fqn()
                } else {
                    format!(
                        "{}{}",
                        if options.class_insert_mode == MixinClassInsertMode::Fqn {
                            entry.fqn()
                        } else {
                            reference.clone()
                        },
                        if context.is_kotlin {
                            "::class"
                        } else {
                            ".class"
                        }
                    )
                };
                let mut value = item(
                    entry.simple_name.clone(),
                    insert,
                    McCompletionKind::Class,
                    format!("0100_{}", entry.simple_name),
                    if strings {
                        "mixin.targets"
                    } else {
                        "mixin.target"
                    },
                );
                value.detail = Some(if strings {
                    entry.fqn()
                } else {
                    entry.package_name.clone()
                });
                value.documentation = Some(entry.internal_name.clone());
                value.filter_text = if strings {
                    format!("{} {}", entry.simple_name, entry.fqn())
                } else {
                    format!(
                        "{} {} {}{}",
                        entry.simple_name,
                        entry.package_name,
                        entry.fqn(),
                        if context.is_kotlin {
                            format!(" {reference}")
                        } else {
                            String::new()
                        }
                    )
                };
                value.metadata.owner = Some(entry.internal_name);
                value.metadata.name = Some(entry.simple_name);
                value.metadata.namespace = Some("NAMED".into());
                value
            })
            .collect()
    }
}

struct AttributeSnippet {
    name: String,
    preview: String,
    insert: String,
}
fn snippet(name: &str, preview: String, insert: String) -> AttributeSnippet {
    AttributeSnippet {
        name: name.into(),
        preview,
        insert,
    }
}
fn quoted(name: &str) -> AttributeSnippet {
    snippet(
        name,
        format!("{name} = \"…\""),
        format!("{name} = \"${{1}}\"$0"),
    )
}
fn quoted_array(name: &str, kotlin: bool) -> AttributeSnippet {
    let (open, close) = if kotlin { ('[', ']') } else { ('{', '}') };
    snippet(
        name,
        format!("{name} = {open} \"…\" {close}"),
        format!("{name} = {open} \"${{1}}\" {close}$0"),
    )
}
fn plain(name: &str) -> AttributeSnippet {
    snippet(name, format!("{name} = …"), format!("{name} = ${{1}}$0"))
}
fn boolean(name: &str) -> AttributeSnippet {
    snippet(
        name,
        format!("{name} = true/false"),
        format!("{name} = ${{1|true,false|}}$0"),
    )
}
fn class_literal(name: &str, kotlin: bool, array: bool) -> AttributeSnippet {
    let suffix = if kotlin { "::class" } else { ".class" };
    if array {
        let (open, close) = if kotlin { ('[', ']') } else { ('{', '}') };
        snippet(
            name,
            format!("{name} = {open} …{suffix} {close}"),
            format!("{name} = {open} ${{1}}{suffix} {close}$0"),
        )
    } else {
        snippet(
            name,
            format!("{name} = …{suffix}"),
            format!("{name} = ${{1}}{suffix}$0"),
        )
    }
}
fn type_reference(ty: &str, kotlin: bool, source: &str) -> String {
    if !kotlin {
        return ty.into();
    }
    let internal = match ty {
        "At.Shift" => "org/spongepowered/asm/mixin/injection/At$Shift".into(),
        "LocalCapture" => "org/spongepowered/asm/mixin/injection/callback/LocalCapture".into(),
        _ => MixinAnnotation::from_simple_name(ty)
            .and_then(|a| a.official_fqns().into_iter().next())
            .unwrap_or_else(|| ty.into())
            .replace('.', "/"),
    };
    let reference =
        MixinImportEditBuilder::reference_for_internal_name(source, &internal, &BTreeSet::new());
    reference.import_fqn.unwrap_or(reference.text)
}
fn enum_value(name: &str, ty: &str, kotlin: bool, source: &str) -> AttributeSnippet {
    let ty = type_reference(ty, kotlin, source);
    snippet(
        name,
        format!("{name} = {ty}.…"),
        format!("{name} = {ty}.${{1}}$0"),
    )
}
fn annotation_value(
    name: &str,
    ty: &str,
    kotlin: bool,
    source: &str,
    array: bool,
) -> AttributeSnippet {
    let reference = type_reference(ty, kotlin, source);
    if kotlin && array {
        snippet(
            name,
            format!("{name} = [ {reference}(…) ]"),
            format!("{name} = [ {reference}(${{1}}) ]$0"),
        )
    } else if kotlin {
        snippet(
            name,
            format!("{name} = {reference}(…)"),
            format!("{name} = {reference}(${{1}})$0"),
        )
    } else {
        snippet(
            name,
            format!("{name} = @{ty}(…)"),
            format!("{name} = @{ty}(${{1}})$0"),
        )
    }
}
fn at_attribute(kotlin: bool, source: &str) -> AttributeSnippet {
    let ty = type_reference("At", kotlin, source);
    if kotlin {
        snippet(
            "at",
            format!("at = [ {ty}(\"…\") ]"),
            format!("at = [ {ty}(\"${{1}}\") ]$0"),
        )
    } else {
        snippet(
            "at",
            "at = @At(\"…\")".into(),
            "at = @At(\"${1}\")$0".into(),
        )
    }
}
fn requirements(constraints: bool) -> Vec<AttributeSnippet> {
    let mut result = vec![
        plain("require"),
        plain("expect"),
        plain("allow"),
        boolean("remap"),
    ];
    if constraints {
        result.push(quoted("constraints"));
    }
    result
}
fn method_base(kotlin: bool) -> Vec<AttributeSnippet> {
    vec![if kotlin {
        quoted_array("method", true)
    } else {
        quoted("method")
    }]
}
fn injector_base(kotlin: bool, source: &str, extras: bool) -> Vec<AttributeSnippet> {
    let mut result = method_base(kotlin);
    result.push(at_attribute(kotlin, source));
    result.push(annotation_value("slice", "Slice", kotlin, source, true));
    result.extend(requirements(!extras));
    if extras {
        result.push(plain("order"));
    }
    result
}
pub struct MixinAttributeCompletionService;
impl MixinAttributeCompletionService {
    pub fn complete(context: &AnnotationContext, source: &str) -> Vec<McCompletionItem> {
        if context.slot != AnnotationSlot::Attribute {
            return vec![];
        }
        let snippets = Self::snippets(context, source);
        let partial = context.partial_value.to_ascii_lowercase();
        snippets
            .into_iter()
            .filter(|s| {
                !context.existing_attributes.contains(&s.name)
                    && s.name.to_ascii_lowercase().starts_with(&partial)
            })
            .filter(|s| {
                !(context.annotation == MixinAnnotation::WrapWithCondition
                    && s.name == "order"
                    && context.resolved_annotation_fqn.as_deref()
                        == Some("com.llamalad7.mixinextras.injector.WrapWithCondition"))
            })
            .filter(|s| {
                context.annotation != MixinAnnotation::WrapOperation
                    || match s.name.as_str() {
                        "at" => !context.existing_attributes.contains("constant"),
                        "constant" => !context.existing_attributes.contains("at"),
                        _ => true,
                    }
            })
            .enumerate()
            .map(|(i, s)| {
                let mut value = item(
                    s.preview,
                    s.insert,
                    McCompletionKind::Keyword,
                    format!("0000_{i:02}_{}", s.name),
                    "mixin.attribute",
                );
                value.detail = Some(format!(
                    "@{} attribute",
                    context
                        .resolved_annotation_fqn
                        .as_deref()
                        .and_then(|s| s.rsplit('.').next())
                        .unwrap_or(context.annotation.simple_name())
                ));
                value.documentation = Some(format!(
                    "Insert the {} attribute and place the cursor in its value.",
                    s.name
                ));
                value.filter_text = s.name.clone();
                value.metadata.name = Some(s.name);
                value.insert_text_format = McCompletionInsertTextFormat::Snippet;
                value
            })
            .collect()
    }
    fn snippets(context: &AnnotationContext, source: &str) -> Vec<AttributeSnippet> {
        let kotlin = context.is_kotlin;
        use MixinAnnotation::*;
        match context.resolved_annotation_fqn.as_deref() {
            Some("io.github.diskria.lapis.annotations.KMixin") => {
                return vec![
                    class_literal("target", true, false),
                    enum_value(
                        "env",
                        "io.github.diskria.lapis.annotations.Env",
                        true,
                        source,
                    ),
                    enum_value(
                        "initStrategy",
                        "io.github.diskria.lapis.annotations.InitStrategy",
                        true,
                        source,
                    ),
                ]
            }
            Some("io.github.diskria.lapis.annotations.KShadow") => {
                return vec![snippet(
                    "modifiers",
                    "modifiers = [ … ]".into(),
                    "modifiers = [ ${1} ]$0".into(),
                )]
            }
            _ => {}
        }
        match context.annotation {
            Mixin => vec![
                class_literal("value", kotlin, kotlin),
                if kotlin {
                    quoted_array("targets", true)
                } else {
                    quoted("targets")
                },
                plain("priority"),
                boolean("remap"),
            ],
            Inject => {
                let mut v = injector_base(kotlin, source, false);
                v.extend([
                    boolean("cancellable"),
                    enum_value("locals", "LocalCapture", kotlin, source),
                    quoted("id"),
                ]);
                v
            }
            Redirect | ModifyArgs => injector_base(kotlin, source, false),
            ModifyArg => {
                let mut v = injector_base(kotlin, source, false);
                v.push(plain("index"));
                v
            }
            ModifyVariable => {
                let mut v = injector_base(kotlin, source, false);
                v.extend([
                    plain("ordinal"),
                    plain("index"),
                    boolean("argsOnly"),
                    boolean("print"),
                ]);
                v
            }
            ModifyConstant => {
                let mut v = method_base(kotlin);
                v.extend([
                    annotation_value("constant", "Constant", kotlin, source, true),
                    annotation_value("slice", "Slice", kotlin, source, true),
                ]);
                v.extend(requirements(true));
                v
            }
            ModifyExpressionValue | ModifyReturnValue | ModifyReceiver | WrapWithCondition => {
                injector_base(kotlin, source, true)
            }
            WrapOperation => {
                let mut v = injector_base(kotlin, source, true);
                v.push(annotation_value(
                    "constant", "Constant", kotlin, source, true,
                ));
                v
            }
            WrapMethod => {
                let mut v = method_base(kotlin);
                v.extend(requirements(false));
                v.push(plain("order"));
                v
            }
            At => vec![
                quoted("value"),
                quoted("target"),
                quoted("slice"),
                plain("ordinal"),
                plain("opcode"),
                enum_value("shift", "At.Shift", kotlin, source),
                plain("by"),
                quoted_array("args", kotlin),
                boolean("remap"),
                quoted("id"),
            ],
            Constant => vec![
                boolean("nullValue"),
                plain("intValue"),
                plain("floatValue"),
                plain("longValue"),
                plain("doubleValue"),
                quoted("stringValue"),
                class_literal("classValue", kotlin, false),
                plain("ordinal"),
                quoted("slice"),
                plain("expandZeroConditions"),
                boolean("log"),
            ],
            Slice => vec![
                quoted("id"),
                annotation_value("from", "At", kotlin, source, false),
                annotation_value("to", "At", kotlin, source, false),
            ],
            Accessor | Invoker => vec![quoted("value"), boolean("remap")],
            Shadow => vec![
                quoted("prefix"),
                quoted_array("aliases", kotlin),
                boolean("remap"),
            ],
            Overwrite => vec![quoted("constraints"), boolean("remap")],
            Local => vec![
                boolean("print"),
                plain("ordinal"),
                plain("index"),
                quoted_array("name", kotlin),
                boolean("argsOnly"),
                class_literal("type", kotlin, false),
            ],
            Share => vec![quoted("value"), quoted("namespace")],
            Definition => {
                let ty = type_reference("Local", kotlin, source);
                vec![
                    quoted("id"),
                    quoted_array("method", kotlin),
                    quoted_array("field", kotlin),
                    class_literal("type", kotlin, true),
                    if kotlin {
                        snippet(
                            "local",
                            format!("local = [ {ty}(…) ]"),
                            format!("local = [ {ty}(${{1}}) ]$0"),
                        )
                    } else {
                        snippet(
                            "local",
                            "local = { @Local(…) }".into(),
                            "local = { @Local(${1}) }$0".into(),
                        )
                    },
                    boolean("remap"),
                ]
            }
            Expression => vec![quoted_array("value", kotlin), quoted("id")],
            Definitions | Expressions => {
                let definition = context.annotation == Definitions;
                let ty = type_reference(
                    if definition {
                        "Definition"
                    } else {
                        "Expression"
                    },
                    kotlin,
                    source,
                );
                let inner = if definition {
                    "id = \"${1}\""
                } else {
                    "\"${1}\""
                };
                vec![if kotlin {
                    snippet(
                        "value",
                        format!("value = [ {ty}(…) ]"),
                        format!("value = [ {ty}({inner}) ]$0"),
                    )
                } else {
                    snippet(
                        "value",
                        format!("value = {{ @{ty}(…) }}"),
                        format!("value = {{ @{ty}({inner}) }}$0"),
                    )
                }]
            }
            Final | Mutable | Pseudo | SoftOverride | Surrogate => vec![],
        }
    }
}

pub struct MixinDeclarationSnippetService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> MixinDeclarationSnippetService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn complete(&self, source: &str, context: &AnnotationContext) -> Vec<McCompletionItem> {
        if context.slot != AnnotationSlot::Handler {
            return vec![];
        }
        let start = if source.as_bytes().get(context.annotation_start_offset) == Some(&b'@') {
            Some(context.annotation_start_offset)
        } else {
            AnnotationContextExtractor::find_annotation_offsets(source, context.annotation)
                .into_iter()
                .filter(|n| *n <= context.value_start_offset.min(source.len()))
                .max()
        };
        let Some(start) = start else {
            return vec![];
        };
        let targets = if context.mixin_target_internal_names.is_empty() {
            MixinTargetResolver::resolve_targets_from_source(source, self.class_index)
        } else {
            context.mixin_target_internal_names.clone()
        };
        self.complete_at(
            source,
            context.annotation,
            start,
            &targets,
            &context.partial_value,
        )
    }
    pub fn complete_at(
        &self,
        source: &str,
        annotation: MixinAnnotation,
        start: usize,
        targets: &[String],
        prefix: &str,
    ) -> Vec<McCompletionItem> {
        if !matches!(
            annotation,
            MixinAnnotation::Accessor | MixinAnnotation::Invoker | MixinAnnotation::Overwrite
        ) || targets.is_empty()
            || !source.is_char_boundary(start)
            || source.as_bytes().get(start) != Some(&b'@')
            || has_following_member(source, start)
        {
            return vec![];
        }
        match annotation {
            MixinAnnotation::Accessor => {
                let Some(name) = annotation_string_value(source, start) else {
                    return vec![];
                };
                if name.is_empty() {
                    return vec![];
                }
                let fields = targets
                    .iter()
                    .map(|owner| {
                        let matches = self
                            .class_index
                            .get_fields(owner)
                            .into_iter()
                            .filter(|f| f.name == name)
                            .collect::<Vec<_>>();
                        (matches.len() == 1).then(|| (owner.clone(), matches[0].clone()))
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(fields) = fields else {
                    return vec![];
                };
                let first = &fields[0].1;
                if fields.iter().any(|(_, f)| {
                    f.descriptor != first.descriptor || f.is_static != first.is_static
                }) {
                    return vec![];
                }
                let mut result = vec![];
                let mut seen = BTreeSet::new();
                for (owner, field) in fields {
                    for setter in [false, true] {
                        if let Some(item) =
                            self.accessor_item(source, start, &owner, &field, setter)
                        {
                            if seen.insert(item.insert_text.clone()) {
                                result.push(item);
                            }
                        }
                    }
                }
                result
            }
            MixinAnnotation::Invoker => {
                let Some(value) = annotation_string_value(source, start) else {
                    return vec![];
                };
                let mut owners = vec![];
                for owner in targets {
                    let Some((name, descriptor)) = self.invoker_selector(&value, owner) else {
                        return vec![];
                    };
                    let methods = self
                        .class_index
                        .get_methods(owner)
                        .into_iter()
                        .filter(|m| {
                            m.name == name
                                && m.name != "<clinit>"
                                && descriptor.as_ref().is_none_or(|d| *d == m.descriptor)
                        })
                        .collect::<Vec<_>>();
                    if methods.is_empty() {
                        return vec![];
                    }
                    owners.push((owner.clone(), methods));
                }
                let first = &owners[0];
                let keys = first
                    .1
                    .iter()
                    .map(|m| (m.descriptor.clone(), m.is_static))
                    .filter(|(descriptor, is_static)| {
                        owners.iter().all(|(_, methods)| {
                            methods
                                .iter()
                                .any(|m| m.descriptor == *descriptor && m.is_static == *is_static)
                        })
                    })
                    .collect::<BTreeSet<_>>();
                keys.into_iter()
                    .filter_map(|(descriptor, is_static)| {
                        first
                            .1
                            .iter()
                            .find(|m| m.descriptor == descriptor && m.is_static == is_static)
                            .and_then(|m| self.method_item(source, start, &first.0, m, false))
                    })
                    .collect()
            }
            MixinAnnotation::Overwrite => {
                if targets.len() != 1 {
                    return vec![];
                }
                let mut methods = self
                    .class_index
                    .get_methods(&targets[0])
                    .into_iter()
                    .filter(|m| {
                        !matches!(m.name.as_str(), "<init>" | "<clinit>")
                            && m.name.starts_with(prefix.trim())
                    })
                    .collect::<Vec<_>>();
                methods.sort_by(|a, b| {
                    (&a.name, &a.descriptor, a.is_static).cmp(&(
                        &b.name,
                        &b.descriptor,
                        b.is_static,
                    ))
                });
                methods.dedup_by(|a, b| {
                    a.name == b.name && a.descriptor == b.descriptor && a.is_static == b.is_static
                });
                methods
                    .iter()
                    .filter_map(|m| self.method_item(source, start, &targets[0], m, true))
                    .collect()
            }
            _ => vec![],
        }
    }
    fn accessor_item(
        &self,
        source: &str,
        start: usize,
        owner: &str,
        field: &FieldIndexEntry,
        setter: bool,
    ) -> Option<McCompletionItem> {
        let parsed = descriptor::parse_field_descriptor(&field.descriptor).ok()?;
        let mut names = BTreeSet::new();
        collect_object_names(&parsed, &mut names);
        let (ty, imports) = render_type(source, &parsed, &names);
        let params = if setter {
            format!("{ty} value")
        } else {
            String::new()
        };
        let signature = format!(
            "{} {} ${{1}}({params})",
            if field.is_static {
                "static"
            } else {
                "public abstract"
            },
            if setter { "void" } else { &ty }
        );
        let insert = if field.is_static {
            body_snippet(&signature)
        } else {
            format!("{signature};$0")
        };
        let mut edits = instance_declaration_edits(source, start, field.is_static)?;
        edits.extend(build_import_edits(source, &imports));
        let action = if setter { "setter" } else { "getter" };
        let mut value = item(
            format!("@Accessor {action} {}: {ty}", field.name),
            insert,
            McCompletionKind::Method,
            format!(
                "0800_accessor_{}_{}",
                field.name,
                if setter { "SETTER" } else { "GETTER" }
            ),
            "mixin.declaration",
        );
        value.detail = Some(format!(
            "{} {}field",
            self.owner_name(owner),
            if field.is_static { "static " } else { "" }
        ));
        value.documentation = Some(signature);
        value.filter_text = format!("{} {action} {ty}", field.name);
        value.metadata.owner = Some(owner.into());
        value.metadata.name = Some(field.name.clone());
        value.metadata.descriptor = Some(field.descriptor.clone());
        value.additional_edits = edits;
        value.insert_text_format = McCompletionInsertTextFormat::Snippet;
        Some(value)
    }
    fn method_item(
        &self,
        source: &str,
        start: usize,
        owner: &str,
        method: &MethodIndexEntry,
        overwrite: bool,
    ) -> Option<McCompletionItem> {
        let parsed = descriptor::parse_method_descriptor(&method.descriptor).ok()?;
        let constructor = method.name == "<init>";
        let returns = if constructor {
            descriptor::JvmType::ObjectType(owner.into())
        } else {
            parsed.return_type.clone()
        };
        let mut names = BTreeSet::new();
        for ty in parsed.parameters.iter().chain([&returns]) {
            collect_object_names(ty, &mut names);
        }
        let mut imports = BTreeSet::new();
        let parameters = parsed
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (ty, fqns) = render_type(source, p, &names);
                imports.extend(fqns);
                format!("{ty} arg{i}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let (returns, return_imports) = render_type(source, &returns, &names);
        imports.extend(return_imports);
        let is_static = constructor || method.is_static;
        let modifier = if overwrite {
            if is_static {
                "public static"
            } else {
                "public"
            }
        } else if is_static {
            "static"
        } else {
            "public abstract"
        };
        let signature = format!("{modifier} {returns} ${{1}}({parameters})");
        let insert = if overwrite && parsed.return_type == descriptor::JvmType::VoidType {
            format!("{signature} {{\n    $0\n}}")
        } else if overwrite || is_static {
            body_snippet(&signature)
        } else {
            format!("{signature};$0")
        };
        let mut edits = if overwrite {
            vec![]
        } else {
            instance_declaration_edits(source, start, is_static)?
        };
        edits.extend(build_import_edits(source, &imports));
        let label = if overwrite {
            format!("@Overwrite {}({parameters}): {returns}", method.name)
        } else if constructor {
            format!("@Invoker constructor {returns}({parameters})")
        } else {
            format!("@Invoker {}({parameters}): {returns}", method.name)
        };
        let mut value = item(
            label,
            insert,
            McCompletionKind::Method,
            format!(
                "{}_{}_{}",
                if overwrite {
                    "0820_overwrite"
                } else {
                    "0810_invoker"
                },
                method.name,
                method.descriptor
            ),
            "mixin.declaration",
        );
        value.detail = Some(format!(
            "{} {}method",
            self.owner_name(owner),
            if is_static { "static " } else { "" }
        ));
        value.documentation = Some(signature);
        value.filter_text = format!("{} {}", method.name, method.readable_signature);
        value.metadata.owner = Some(owner.into());
        value.metadata.name = Some(method.name.clone());
        value.metadata.descriptor = Some(method.descriptor.clone());
        value.additional_edits = edits;
        value.insert_text_format = McCompletionInsertTextFormat::Snippet;
        Some(value)
    }
    fn owner_name(&self, owner: &str) -> String {
        self.class_index
            .find_class(owner)
            .map(|c| c.fqn())
            .unwrap_or_else(|| owner.replace('/', ".").replace('$', "."))
    }
    fn invoker_selector(&self, value: &str, owner: &str) -> Option<(String, Option<String>)> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let (name, descriptor) = if let Some(i) = value.find('(') {
            descriptor::parse_method_descriptor(&value[i..]).ok()?;
            (&value[..i], Some(value[i..].into()))
        } else {
            (value, None)
        };
        let name = name.rsplit([';', '.']).next()?.trim();
        if name.is_empty() {
            return None;
        }
        let simple = self
            .class_index
            .find_class(owner)
            .map(|c| c.simple_name)
            .unwrap_or_else(|| owner.rsplit(['/', '$']).next().unwrap_or(owner).into());
        Some((
            if name == simple {
                "<init>".into()
            } else {
                name.into()
            },
            descriptor,
        ))
    }
}
fn body_snippet(signature: &str) -> String {
    format!("{signature} {{\n    $0\n    throw new AssertionError();\n}}")
}
fn collect_object_names(ty: &descriptor::JvmType, names: &mut BTreeSet<String>) {
    match ty {
        descriptor::JvmType::ObjectType(n) => {
            names.insert(n.clone());
        }
        descriptor::JvmType::ArrayType(c) => collect_object_names(c, names),
        _ => {}
    }
}
fn render_type(
    source: &str,
    ty: &descriptor::JvmType,
    names: &BTreeSet<String>,
) -> (String, BTreeSet<String>) {
    match ty {
        descriptor::JvmType::ObjectType(n) => {
            let reference = MixinImportEditBuilder::reference_for_internal_name(source, n, names);
            (reference.text, reference.import_fqn.into_iter().collect())
        }
        descriptor::JvmType::ArrayType(c) => {
            let (text, imports) = render_type(source, c, names);
            (format!("{text}[]"), imports)
        }
        _ => (descriptor::render(ty), BTreeSet::new()),
    }
}
fn build_import_edits(source: &str, imports: &BTreeSet<String>) -> Vec<McTextEdit> {
    let edits = imports
        .iter()
        .filter_map(|fqn| MixinImportEditBuilder::build_import_edit(source, fqn))
        .collect::<Vec<_>>();
    let Some(first) = edits.first() else {
        return vec![];
    };
    let newline = if first.new_text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let leading = if first.new_text.starts_with(newline) {
        newline
    } else {
        ""
    };
    vec![McTextEdit {
        start_offset: first.start_offset,
        end_offset: first.end_offset,
        new_text: leading.to_owned()
            + &edits
                .iter()
                .map(|e| e.new_text.trim_start_matches(['\r', '\n']))
                .collect::<String>(),
    }]
}
fn annotation_string_value(source: &str, start: usize) -> Option<String> {
    let tail = &source[start..];
    let site = annotations(tail).into_iter().find(|a| a.start == 0)?;
    if tail.as_bytes().get(site.body_end) != Some(&b')') {
        return None;
    }
    annotation_values(tail, &site, "value")
        .first()
        .map(|v| v.value.clone())
}
fn annotation_token_end(
    tokens: &[crate::mixin_source::SourceToken],
    start: usize,
) -> Option<usize> {
    if tokens.get(start)?.text != "@" {
        return None;
    }
    let mut i = start + 1;
    if tokens.get(i)?.kind != SourceTokenKind::Identifier {
        return None;
    }
    i += 1;
    while tokens.get(i).is_some_and(|t| t.text == ".")
        && tokens
            .get(i + 1)
            .is_some_and(|t| t.kind == SourceTokenKind::Identifier)
    {
        i += 2;
    }
    if tokens.get(i).is_some_and(|t| t.text == "(") {
        Some(matching(tokens, i)? + 1)
    } else {
        Some(i)
    }
}
fn has_following_member(source: &str, start: usize) -> bool {
    let ts = source_tokens(&source[start..], true);
    let Some(mut i) = annotation_token_end(&ts, 0) else {
        return false;
    };
    while ts.get(i).is_some_and(|t| t.text == "@") {
        let Some(end) = annotation_token_end(&ts, i) else {
            return false;
        };
        i = end;
    }
    ts.get(i).is_some_and(|t| t.text != "}")
}
fn instance_declaration_edits(
    source: &str,
    offset: usize,
    is_static: bool,
) -> Option<Vec<McTextEdit>> {
    if is_static {
        return Some(vec![]);
    }
    let tokens = source_tokens(source, true);
    let mut enclosing = vec![];
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != SourceTokenKind::Identifier
            || !matches!(t.text.as_str(), "class" | "interface" | "record" | "enum")
            || !tokens
                .get(i + 1)
                .is_some_and(|n| n.kind == SourceTokenKind::Identifier)
        {
            continue;
        }
        let Some(open) = (i + 2..tokens.len()).find(|n| tokens[*n].text == "{") else {
            continue;
        };
        if tokens[open].start >= offset {
            continue;
        }
        let Some(close) = matching(&tokens, open) else {
            continue;
        };
        if tokens[close].start <= offset {
            continue;
        }
        let boundary = tokens[..i]
            .iter()
            .rposition(|t| matches!(t.text.as_str(), "{" | "}" | ";"))
            .map_or(0, |n| n + 1);
        let modifiers = &tokens[boundary..i];
        enclosing.push((
            tokens[close].end - tokens[open].start,
            t,
            modifiers.iter().any(|t| t.text == "abstract"),
            modifiers.iter().any(|t| t.text == "final"),
        ));
    }
    let Some((_, kind, is_abstract, is_final)) =
        enclosing.into_iter().min_by_key(|(size, _, _, _)| *size)
    else {
        return Some(vec![]);
    };
    if kind.text == "interface" {
        return Some(vec![]);
    }
    if kind.text != "class" || is_final {
        return None;
    }
    if is_abstract {
        return Some(vec![]);
    }
    Some(vec![McTextEdit {
        start_offset: kind.start,
        end_offset: kind.end,
        new_text: "abstract class".into(),
    }])
}
