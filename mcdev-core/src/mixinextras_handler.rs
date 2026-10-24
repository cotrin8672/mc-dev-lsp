//! Handler signature layouts, source facts and validation for Minecraft injectors.
use crate::{
    bytecode::*,
    descriptor::{self, MemberTarget},
    extras_model::*,
    mixin_source::*,
    mixinextras_operation::OperationSignatureRenderer,
    *,
};
use std::collections::{BTreeMap, BTreeSet};
const OPERATION_FQN: &str = "com.llamalad7.mixinextras.injector.wrapoperation.Operation";
const COERCE_FQN: &str = "org.spongepowered.asm.mixin.injection.Coerce";
const CANCELLABLE_FQN: &str = "com.llamalad7.mixinextras.sugar.Cancellable";
const CALLBACK_INFO: &str = "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;";
const CALLBACK_INFO_RETURNABLE: &str =
    "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;";
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HandlerValidationIssue {
    pub code: String,
    pub message: String,
    pub range: McTextRange,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WrapWithConditionTargetStatus {
    ValidVoid,
    ValidPoppedNonVoid,
    InvalidRetainedNonVoid,
    InvalidInstruction,
    Unresolved,
}
/// Host facts replace the legacy VM reflection fallback. None means unavailable.
pub trait HandlerReferenceTypeResolver: Send + Sync {
    fn is_known_reference(&self, internal: &str) -> Option<bool>;
    /// Java `Class(from_internal).isAssignableFrom(Class(expected_internal))`:
    /// the first argument receives a value of the second type. A wire API whose
    /// `from` means the value type must reverse these arguments at its boundary.
    fn is_assignable(&self, from_internal: &str, expected_internal: &str) -> Option<bool>;
}
pub struct HandlerSignatureService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: Option<&'a dyn BytecodeIndex>,
    pub reference_types: Option<&'a dyn HandlerReferenceTypeResolver>,
}
#[derive(Clone, PartialEq, Eq)]
struct WrappedTarget {
    owner: String,
    parameters: Vec<String>,
    return_type: String,
    is_static: bool,
}
struct FieldTarget {
    owner: String,
    descriptor: String,
    operation: AtTargetOperationKind,
}
impl<'a> HandlerSignatureService<'a> {
    pub fn new(
        class_index: &'a dyn ClassIndex,
        bytecode_index: Option<&'a dyn BytecodeIndex>,
    ) -> Self {
        Self {
            class_index,
            bytecode_index,
            reference_types: None,
        }
    }
    pub fn with_reference_types(mut self, resolver: &'a dyn HandlerReferenceTypeResolver) -> Self {
        self.reference_types = Some(resolver);
        self
    }
    pub fn generate_handler_stub(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        targets: &[String],
        method_name: &str,
        indent: &str,
        context: Option<&ExpressionContext>,
        signature_spec: Option<&HandlerSignatureSpec>,
    ) -> Option<String> {
        let target = self.resolve_target_method(targets, &site.method_attribute)?;
        let owned;
        let spec = if let Some(spec) = signature_spec {
            spec
        } else {
            owned = self.expected_signature(source, site, targets, context)?;
            &owned
        };
        let params = spec
            .parameters
            .iter()
            .map(|p| format!("{} {}", p.readable_type, p.name))
            .collect::<Vec<_>>()
            .join(", ");
        use MixinExtrasAnnotation::*;
        let body = match site.annotation {
            WrapWithCondition => format!("{indent}return true;\n"),
            WrapOperation | WrapMethod => format!(
                "{indent}{}{}.call({});\n",
                if spec.return_type_descriptor == "V" {
                    ""
                } else {
                    "return "
                },
                spec.parameters
                    .last()
                    .map_or("original", |p| p.name.as_str()),
                spec.operation_call_args.join(", ")
            ),
            Redirect => format!("{indent}throw new UnsupportedOperationException();\n"),
            ModifyArg | ModifyVariable
                if spec.return_type_descriptor == "V" || spec.parameters.is_empty() =>
            {
                format!("{indent}// TODO\n")
            }
            ModifyArg
            | ModifyVariable
            | ModifyExpressionValue
            | ModifyConstant
            | ModifyReturnValue
            | ModifyReceiver
                if !spec.parameters.is_empty() =>
            {
                format!("{indent}return {};\n", spec.parameters[0].name)
            }
            _ => format!("{indent}// TODO\n"),
        };
        Some(format!(
            "{indent}{}{} {method_name}({params}) {{\n{body}{indent}}}\n",
            if target.is_static { "static " } else { "" },
            spec.readable_return_type
        ))
    }
    pub fn validate_handler(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        targets: &[String],
        handler: &HandlerMethodDeclaration,
        context: Option<&ExpressionContext>,
    ) -> Vec<HandlerValidationIssue> {
        use MixinExtrasAnnotation::*;
        let resolution = self.resolve_target_resolution(targets, &site.method_attribute);
        let target = resolution.as_ref().ok().and_then(|m| m.as_ref());
        let mut issues =
            self.validate_common_sugar_constraints(handler, target, Some(site.annotation));
        let issue = |code: &str, message: String, range: McTextRange| HandlerValidationIssue {
            code: code.into(),
            message,
            range,
        };
        if site.annotation == WrapMethod {
            for p in &handler.parameters {
                if !p.is_sugar {
                    continue;
                }
                let sugar = match p.sugar_spec {
                    Some(HandlerParameterSugarSpec::Local(_)) => "@Local",
                    Some(HandlerParameterSugarSpec::Cancellable) => "@Cancellable",
                    _ => continue,
                };
                issues.push(issue(
                    diagnostic_codes::UNSUPPORTED_SUGAR_PARAMETER,
                    format!("WrapMethod does not support {sugar}; only @Share is supported"),
                    p.sugar_annotation_range
                        .or(p.range)
                        .unwrap_or(handler.range),
                ));
            }
        }
        if resolution.is_err() {
            issues.push(issue(
                diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                format!(
                    "Ambiguous target method '{}'; an explicit method descriptor is required",
                    site.method_attribute.split('(').next().unwrap_or("")
                ),
                handler.range,
            ));
            return issues;
        }
        if site.annotation == WrapMethod {
            if let Some(target) = target {
                if ["<init>", "<clinit>"].contains(&target.name.as_str()) {
                    issues.push(issue(
                        diagnostic_codes::WRAP_METHOD_INVALID_TARGET,
                        "WrapMethod cannot target an initializer method".into(),
                        handler.range,
                    ));
                    return issues;
                }
                if handler.is_static != target.is_static {
                    issues.push(issue(
                        diagnostic_codes::WRAP_METHOD_STATIC_MISMATCH,
                        format!(
                            "WrapMethod handler must be {} to match the target method",
                            if target.is_static {
                                "static"
                            } else {
                                "instance"
                            }
                        ),
                        handler.range,
                    ));
                    return issues;
                }
            }
        }
        if site.annotation == ModifyReturnValue
            && target.is_some_and(|m| return_descriptor(&m.descriptor) == "V")
        {
            issues.push(issue(
                diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                "ModifyReturnValue cannot target a void method".into(),
                handler.range,
            ));
            return issues;
        }
        let Some(expected) = self.expected_signature(source, site, targets, context) else {
            return issues;
        };
        // A missing host hierarchy fact is not evidence of a type mismatch.
        if handler.parameters.iter().any(|p| {
            !p.is_sugar
                && !p.is_operation
                && p.type_descriptor.is_none()
                && self.unresolved_type_is_unavailable(&p.type_name)
        }) {
            return issues;
        }
        if !int_like_types_match(&expected, handler) {
            issues.push(issue(
                diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                "All int-like handler types must use one type (boolean, byte, char, short, or int)"
                    .into(),
                handler.range,
            ));
        }
        if handler.return_type_descriptor.is_some() {
            let matched = self.type_match_knowledge(
                handler.return_type_descriptor.as_deref(),
                &expected.return_type_descriptor,
                handler.has_method_level_coerce,
                &expected.accepted_return_type_descriptors,
            );
            if matched.is_none() {
                return issues;
            }
            if matched == Some(false) {
                issues.push(issue(
                    diagnostic_codes::WRONG_RETURN_TYPE,
                    format!(
                        "MixinExtras handler return type should be {}",
                        expected.readable_return_type
                    ),
                    handler.range,
                ));
            }
        }
        let operations: Vec<_> = handler
            .parameters
            .iter()
            .filter(|p| p.is_operation)
            .collect();
        match site.annotation {
            WrapWithCondition if !operations.is_empty() => {
                issues.push(issue(
                    diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                    "WrapWithCondition handler must not declare an Operation parameter".into(),
                    handler.range,
                ));
                return issues;
            }
            WrapOperation | WrapMethod => {
                let Some(actual) = operations.last() else {
                    issues.push(issue(
                        diagnostic_codes::MISSING_OPERATION_PARAMETER,
                        "MixinExtras handler is missing Operation parameter".into(),
                        handler.range,
                    ));
                    return issues;
                };
                if let Some(operation) = expected.parameters.iter().rev().find(|p| p.is_operation) {
                    if !operation_generic_matches(operation, actual) {
                        issues.push(issue(
                            diagnostic_codes::WRONG_OPERATION_GENERIC,
                            format!(
                                "Operation generic should be {}",
                                OperationSignatureRenderer::readable_type(
                                    operation.operation_generic_descriptor.as_deref().unwrap()
                                )
                            ),
                            handler.range,
                        ));
                    }
                }
                let base: Vec<_> = handler.parameters.iter().filter(|p| !p.is_sugar).collect();
                if site.annotation == WrapMethod && base.last() != Some(actual) {
                    issues.push(issue(
                        diagnostic_codes::MISSING_OPERATION_PARAMETER,
                        "Operation<T> must be the last handler parameter".into(),
                        handler.range,
                    ));
                }
                if site.annotation == WrapOperation
                    && (base.len() < expected.parameters.len()
                        || !base[expected.parameters.len() - 1].is_operation)
                {
                    issues.push(issue(
                        diagnostic_codes::MISSING_OPERATION_PARAMETER,
                        "Operation<T> must be the last required wrapped-operation parameter".into(),
                        handler.range,
                    ));
                }
            }
            ModifyExpressionValue | ModifyReturnValue => {
                if let Some(original) = expected.parameters.first() {
                    let matched = handler
                        .parameters
                        .iter()
                        .find(|p| !p.is_sugar)
                        .map_or(Some(false), |actual| {
                            self.parameter_match_knowledge(original, actual)
                        });
                    if matched.is_none() {
                        return issues;
                    }
                    if matched == Some(false) {
                        issues.push(issue(
                            diagnostic_codes::WRONG_ORIGINAL_VALUE_TYPE,
                            format!(
                                "Original value parameter should be {}",
                                original.readable_type
                            ),
                            handler.range,
                        ));
                    }
                }
            }
            _ => {}
        }
        if issues.is_empty()
            && self.parameters_match(site.annotation, &expected, handler) == Some(false)
        {
            issues.push(issue(
                diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                "MixinExtras handler signature mismatch".into(),
                handler.range,
            ));
        }
        issues
    }
    fn parameters_match(
        &self,
        annotation: MixinExtrasAnnotation,
        expected: &HandlerSignatureSpec,
        handler: &HandlerMethodDeclaration,
    ) -> Option<bool> {
        if !sugar_is_trailing(&handler.parameters) {
            return Some(false);
        }
        let base: Vec<_> = handler.parameters.iter().filter(|p| !p.is_sugar).collect();
        use MixinExtrasAnnotation::*;
        let required = match annotation {
            ModifyReturnValue | ModifyExpressionValue => 1,
            _ => expected.parameters.len(),
        };
        let captured_allowed = matches!(
            annotation,
            ModifyReturnValue
                | ModifyExpressionValue
                | ModifyReceiver
                | WrapOperation
                | WrapWithCondition
        );
        if base.len() < required
            || expected.parameters.len() < required
            || (!captured_allowed && base.len() != required)
        {
            return Some(false);
        }
        for i in 0..required {
            if !self.parameter_match_knowledge(&expected.parameters[i], base[i])? {
                return Some(false);
            }
        }
        if !captured_allowed {
            return Some(true);
        }
        let captured = &base[required..];
        if captured.len() > expected.optional_captured_target_parameters.len() {
            return Some(false);
        }
        for (actual, exp) in captured
            .iter()
            .zip(&expected.optional_captured_target_parameters)
        {
            if !self.type_match_knowledge(
                actual.type_descriptor.as_deref(),
                &exp.type_descriptor,
                actual.has_coerce,
                &BTreeSet::new(),
            )? {
                return Some(false);
            }
        }
        Some(true)
    }
    fn unresolved_type_is_unavailable(&self, name: &str) -> bool {
        let Some(resolver) = self.reference_types else {
            return true;
        };
        let mut name = name.trim();
        while let Some(rest) = name.strip_suffix("[]") {
            name = rest.trim();
        }
        let name = name.split('<').next().unwrap_or("").trim();
        let candidates = if name.contains('.') {
            vec![name.replace('.', "/")]
        } else {
            vec![format!("java/lang/{name}"), name.into()]
        };
        !candidates
            .iter()
            .all(|name| resolver.is_known_reference(name) == Some(false))
    }
    pub fn parameter_match_knowledge(
        &self,
        expected: &HandlerParameterSpec,
        actual: &HandlerParameterDeclaration,
    ) -> Option<bool> {
        if expected.is_operation {
            return Some(
                actual.is_operation
                    && expected.operation_generic_descriptor.is_some()
                    && operation_generic_matches(expected, actual),
            );
        }
        if actual.type_descriptor.is_none()
            && self.unresolved_type_is_unavailable(&actual.type_name)
        {
            return None;
        }
        self.type_match_knowledge(
            actual.type_descriptor.as_deref(),
            &expected.type_descriptor,
            actual.has_coerce,
            &expected.accepted_type_descriptors,
        )
    }
    fn resolve_target_resolution(
        &self,
        targets: &[String],
        attribute: &str,
    ) -> Result<Option<MethodIndexEntry>, ()> {
        let open = attribute.find('(');
        let name = open.map_or(attribute, |i| &attribute[..i]);
        let explicit = open.map(|i| &attribute[i..]);
        let owners = self.owners(targets);
        if owners.is_empty() {
            return Ok(None);
        }
        let matches: Vec<Vec<_>> = owners
            .into_iter()
            .map(|owner| {
                self.class_index
                    .get_methods(&owner)
                    .into_iter()
                    .filter(|m| m.name == name)
                    .collect()
            })
            .collect();
        if matches.iter().any(Vec::is_empty) {
            return Ok(None);
        }
        let matches: Vec<Vec<_>> = matches
            .into_iter()
            .map(|methods| {
                methods
                    .into_iter()
                    .filter(|m| explicit.is_none_or(|d| m.descriptor == d))
                    .collect()
            })
            .collect();
        if matches.iter().any(Vec::is_empty) {
            return Ok(None);
        }
        if matches.iter().any(|m| m.len() != 1) {
            return Err(());
        }
        let first = matches[0][0].clone();
        if matches
            .iter()
            .any(|m| m[0].descriptor != first.descriptor || m[0].is_static != first.is_static)
        {
            return Err(());
        }
        Ok(Some(first))
    }
    pub fn type_match_knowledge(
        &self,
        actual: Option<&str>,
        expected: &str,
        coerce: bool,
        accepted: &BTreeSet<String>,
    ) -> Option<bool> {
        let Some(actual) = actual else {
            return Some(false);
        };
        if actual == expected || accepted.contains(actual) {
            return Some(true);
        }
        if !coerce {
            return Some(false);
        }
        if descriptor::parse_field_descriptor(actual).is_err()
            || descriptor::parse_field_descriptor(expected).is_err()
        {
            return Some(false);
        }
        let a = actual.bytes().take_while(|b| *b == b'[').count();
        let b = expected.bytes().take_while(|b| *b == b'[').count();
        if a > 0 || b > 0 {
            if a != b || a == 0 || !actual[a..].starts_with('L') || !expected[b..].starts_with('L')
            {
                return Some(false);
            }
            return self.coerce_reference_knowledge(
                &actual[a + 1..actual.len() - 1],
                &expected[b + 1..expected.len() - 1],
            );
        }
        if !actual.starts_with('L') || !expected.starts_with('L') {
            return Some(["B", "S", "C", "Z"].contains(&actual) && expected == "I");
        }
        self.coerce_reference_knowledge(
            &actual[1..actual.len() - 1],
            &expected[1..expected.len() - 1],
        )
    }
    pub fn resolve_target_method(
        &self,
        mixin_targets: &[String],
        method_attribute: &str,
    ) -> Option<MethodIndexEntry> {
        self.resolve_target_resolution(mixin_targets, method_attribute)
            .ok()
            .flatten()
    }
    pub fn parse_handler_method(
        source: &str,
        start_offset: usize,
        imports: Option<&JavaSourceImports>,
    ) -> Option<HandlerMethodDeclaration> {
        let owned;
        let imports = if let Some(imports) = imports {
            imports
        } else {
            owned = JavaTypeDescriptorResolver::imports_for(source);
            &owned
        };
        let member = parse_member_declaration(source, start_offset, imports)?;
        if !member.is_method {
            return None;
        }
        let param_start = member.parameter_start?;
        let param_end = member.parameter_end?;
        if !imports.is_kotlin {
            let allowed = [
                "public",
                "protected",
                "private",
                "static",
                "final",
                "synchronized",
                "native",
                "strictfp",
                "default",
            ];
            let masked =
                AnnotationContextExtractor::mask_non_code(&source[member.start..member.name_start]);
            let header = masked.trim_end();
            let prefix = header.strip_suffix(member.return_type.as_str())?.trim();
            if member.return_type.chars().any(char::is_whitespace)
                || prefix.split_whitespace().any(|m| !allowed.contains(&m))
            {
                return None;
            }
        }
        let method_annotations =
            strip_parameter_annotations(&source[start_offset..member.start], imports);
        let end = if imports.is_kotlin {
            member.end
        } else {
            param_end + 1
        };
        Some(HandlerMethodDeclaration {
            method_name: member.name,
            return_type_name: member.return_type,
            return_type_descriptor: None,
            parameters: parse_parameters(source, param_start, param_end, imports),
            range: source_range(source, member.start, end),
            is_static: if imports.is_kotlin {
                Self::is_kotlin_companion_member(source, member.start)
            } else {
                member.modifiers.contains(&JavaModifier::Static)
            },
            has_method_level_coerce: method_annotations.has_coerce,
        })
    }
    pub fn is_kotlin_companion_member(source: &str, offset: usize) -> bool {
        let Some(prefix) = source.get(..offset) else {
            return false;
        };
        let tokens = source_tokens(prefix, true);
        let mut companion_braces = BTreeSet::new();
        for (i, t) in tokens.iter().enumerate() {
            if t.text == "companion" && tokens.get(i + 1).is_some_and(|t| t.text == "object") {
                if let Some(brace) = tokens
                    .iter()
                    .skip(i + 2)
                    .take_while(|t| t.text != "}")
                    .find(|t| t.text == "{")
                {
                    companion_braces.insert(brace.start);
                }
            }
        }
        let mut scopes = vec![];
        for t in tokens
            .into_iter()
            .filter(|t| t.kind == SourceTokenKind::Symbol)
        {
            if t.text == "{" {
                scopes.push(companion_braces.contains(&t.start));
            } else if t.text == "}" {
                scopes.pop();
            }
        }
        scopes.last() == Some(&true)
    }
    pub fn enrich_handler_types(
        handler: &HandlerMethodDeclaration,
        class_index: &dyn ClassIndex,
        source: Option<&str>,
    ) -> HandlerMethodDeclaration {
        Self::enrich_handler_types_with_reference_types(handler, class_index, source, None)
    }
    /// Supply SDK/classpath facts for the legacy Class.forName fallback. Without
    /// those facts an unresolved type remains None instead of becoming Object.
    pub fn enrich_handler_types_with_reference_types(
        handler: &HandlerMethodDeclaration,
        class_index: &dyn ClassIndex,
        source: Option<&str>,
        reference_types: Option<&dyn HandlerReferenceTypeResolver>,
    ) -> HandlerMethodDeclaration {
        let imports = source.map(JavaTypeDescriptorResolver::imports_for);
        let kotlin = imports.as_ref().is_some_and(|i| i.is_kotlin);
        let mut result = handler.clone();
        result.return_type_descriptor = if kotlin {
            handler.return_type_descriptor.clone().or_else(|| {
                let imports = imports.as_ref().unwrap();
                JavaTypeDescriptorResolver::method_descriptor_or_null(
                    &handler.return_type_name,
                    &[],
                    &JavaTypeResolutionContext {
                        imports,
                        lookup: Some(&ClassIndexJavaTypeLookup { class_index }),
                    },
                )
                .and_then(|s| s.split_once(')').map(|(_, r)| r.into()))
            })
        } else {
            descriptor_from_handler_type(
                &handler.return_type_name,
                class_index,
                imports.as_ref(),
                reference_types,
            )
        };
        for p in &mut result.parameters {
            p.type_descriptor = if p.is_operation {
                Some(format!("L{};", OPERATION_FQN.replace('.', "/")))
            } else if kotlin && p.type_descriptor.is_some() {
                p.type_descriptor.clone()
            } else {
                descriptor_from_handler_type(
                    &p.type_name,
                    class_index,
                    imports.as_ref(),
                    reference_types,
                )
            };
            if kotlin && p.is_operation {
                p.operation_generic_descriptor =
                    p.operation_generic_descriptor.clone().or_else(|| {
                        p.operation_generic_name
                            .as_ref()
                            .and_then(|name| {
                                descriptor_from_handler_type(
                                    name,
                                    class_index,
                                    imports.as_ref(),
                                    reference_types,
                                )
                            })
                            .map(|d| {
                                boxed_generic(&d)
                                    .map_or(d.clone(), |name| format!("Ljava/lang/{name};"))
                            })
                    });
                if p.operation_generic_descriptor.is_none() {
                    p.operation_generic_name = None;
                }
            }
        }
        result
    }
    pub fn share_ref_value_descriptor(descriptor: &str) -> Option<&'static str> {
        let class = descriptor
            .strip_prefix('L')?
            .strip_suffix(';')?
            .strip_prefix("com/llamalad7/mixinextras/sugar/ref/")?;
        Some(match class {
            "LocalRef" => "Ljava/lang/Object;",
            "LocalBooleanRef" => "Z",
            "LocalByteRef" => "B",
            "LocalCharRef" => "C",
            "LocalShortRef" => "S",
            "LocalIntRef" => "I",
            "LocalLongRef" => "J",
            "LocalFloatRef" => "F",
            "LocalDoubleRef" => "D",
            _ => return None,
        })
    }
    pub fn find_annotation_sites(source: &str) -> Vec<MixinExtrasAnnotationSite> {
        find_sites(source, false)
    }
    pub fn find_sugar_handler_annotation_sites(source: &str) -> Vec<MixinExtrasAnnotationSite> {
        find_sites(source, true)
    }
    fn owners(&self, targets: &[String]) -> Vec<String> {
        MixinTargetResolver::resolve_targets(targets, self.class_index, None)
    }
    pub fn expected_signature(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        targets: &[String],
        context: Option<&ExpressionContext>,
    ) -> Option<HandlerSignatureSpec> {
        use crate::mixinextras_expression::ExpressionContextResolver as Expression;
        use MixinExtrasAnnotation::*;
        let method = self.resolve_target_method(targets, &site.method_attribute)?;
        let at = site.at_value.as_deref().unwrap_or("");
        if method.name == "<init>"
            && !((site.annotation == Inject && at.eq_ignore_ascii_case("RETURN"))
                || (site.annotation == WrapOperation && at.eq_ignore_ascii_case("NEW")))
        {
            return None;
        }
        if site.at_args.iter().any(|s| {
            s.split('=')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("array")
        }) {
            return None;
        }
        match site.annotation {
            Inject => {
                if at.trim().is_empty()
                    || annotation_value(source, site, "locals")
                        .is_some_and(|s| s.rsplit('.').next().unwrap_or("").trim() != "NO_CAPTURE")
                {
                    return None;
                }
                let ret = return_descriptor(&method.descriptor);
                let mut params = parameters(&parameter_descriptors(&method.descriptor));
                let mut callback = parameter(
                    "ci",
                    if ret == "V" {
                        CALLBACK_INFO
                    } else {
                        CALLBACK_INFO_RETURNABLE
                    },
                );
                if ret != "V" {
                    let boxed = boxed_generic(ret)
                        .map_or_else(|| ret.into(), |name| format!("Ljava/lang/{name};"));
                    callback.readable_type = format!(
                        "CallbackInfoReturnable<{}>",
                        OperationSignatureRenderer::readable_type(&boxed)
                    );
                    callback.generic_type_descriptor = Some(boxed);
                }
                params.push(callback);
                Some(signature("V", params))
            }
            ModifyArgs => (at.eq_ignore_ascii_case("INVOKE")).then(|| {
                signature(
                    "V",
                    vec![parameter(
                        "args",
                        "Lorg/spongepowered/asm/mixin/injection/invoke/arg/Args;",
                    )],
                )
            }),
            ModifyVariable => self.expected_variable(source, site, &method, targets),
            ModifyConstant => {
                if !site.expand_zero_conditions.is_empty()
                    && site
                        .at_args
                        .iter()
                        .any(|s| s.split('=').next() != Some("intValue"))
                {
                    return None;
                }
                let descriptor = if site.at_args.is_empty() {
                    if site.expand_zero_conditions.is_empty() {
                        return None;
                    }
                    "I".into()
                } else {
                    crate::mixinextras_occurrence::ConstantAtArgsParser::parse(&site.at_args)?
                };
                Some(original_signature(&descriptor))
            }
            ModifyReturnValue => {
                let ret = return_descriptor(&method.descriptor);
                if ret == "V" {
                    return None;
                }
                let mut spec = original_signature(ret);
                spec.optional_captured_target_parameters =
                    parameters(&parameter_descriptors(&method.descriptor));
                Some(spec)
            }
            WrapMethod => {
                if ["<init>", "<clinit>"].contains(&method.name.as_str())
                    || descriptor::parse_method_descriptor(&method.descriptor).is_err()
                {
                    return None;
                }
                Some(wrap_signature(
                    &WrappedTarget {
                        owner: String::new(),
                        parameters: parameter_descriptors(&method.descriptor),
                        return_type: return_descriptor(&method.descriptor).into(),
                        is_static: true,
                    },
                    None,
                ))
            }
            ModifyExpressionValue => {
                let (info, int_like) = if at.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION") {
                    let info = Expression::infer_expression_value_type_info(
                        source,
                        site,
                        &method,
                        targets,
                        self.bytecode_index?,
                        self.class_index,
                        context,
                    )?;
                    (info.descriptor, info.int_like)
                } else {
                    (
                        self.infer_expression_value_type(source, site, &method, targets, context)?,
                        false,
                    )
                };
                let mut spec = original_signature(&info);
                spec.optional_captured_target_parameters =
                    parameters(&parameter_descriptors(&method.descriptor));
                if int_like {
                    spec.accepted_return_type_descriptors = int_like_descriptors();
                    spec.parameters[0].accepted_type_descriptors = int_like_descriptors();
                }
                Some(spec)
            }
            ModifyArg => {
                if !at.eq_ignore_ascii_case("INVOKE") {
                    return None;
                }
                let wrapped = self.operation_invoke(
                    site.at_target.as_deref(),
                    &method,
                    targets,
                    site.at_ordinal,
                )?;
                if !self.known_method_target(&wrapped) {
                    return None;
                }
                let index = annotation_value(source, site, "index")
                    .and_then(|v| v.trim().parse::<i32>().ok())
                    .or_else(|| (wrapped.parameters.len() == 1).then_some(0))?;
                Some(original_signature(
                    wrapped.parameters.get(usize::try_from(index).ok()?)?,
                ))
            }
            Redirect => {
                if at.eq_ignore_ascii_case("NEW") {
                    let mut operation = self.expected_new(site, &method, targets)?;
                    if !operation
                        .parameters
                        .last()
                        .is_some_and(|p| p.is_operation && p.operation_generic_descriptor.is_some())
                    {
                        return None;
                    }
                    operation.parameters.pop();
                    operation.operation_call_args.clear();
                    return Some(operation);
                }
                let wrapped = if at.eq_ignore_ascii_case("INVOKE") {
                    let wrapped = self.operation_invoke(
                        site.at_target.as_deref(),
                        &method,
                        targets,
                        site.at_ordinal,
                    )?;
                    if !self.known_method_target(&wrapped) {
                        return None;
                    }
                    wrapped
                } else if at.eq_ignore_ascii_case("FIELD") {
                    field_wrapped(self.parse_field(
                        site.at_target.as_deref(),
                        &method,
                        targets,
                        field_operations(),
                        site.at_ordinal,
                    )?)
                } else {
                    return None;
                };
                let mut params = wrapped_parameters(&wrapped);
                if at.eq_ignore_ascii_case("FIELD") && wrapped.return_type == "V" {
                    if let Some(last) = params.last_mut() {
                        last.name = "value".into();
                    }
                }
                Some(signature(&wrapped.return_type, params))
            }
            ModifyReceiver => {
                let wrapped = if at.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION") {
                    let layout = Expression::infer_modify_receiver_layout(
                        source,
                        site,
                        &method,
                        targets,
                        self.bytecode_index?,
                        self.class_index,
                        context,
                    )?;
                    WrappedTarget {
                        owner: layout.receiver_owner_internal_name,
                        parameters: layout.parameter_descriptors,
                        return_type: "V".into(),
                        is_static: false,
                    }
                } else {
                    self.modify_receiver_target(site.at_target.as_deref(), &method, targets)?
                };
                let mut spec = signature(
                    &format!("L{};", wrapped.owner),
                    wrapped_parameters(&wrapped),
                );
                spec.optional_captured_target_parameters =
                    parameters(&parameter_descriptors(&method.descriptor));
                Some(spec)
            }
            WrapOperation => {
                let has_at = site.at_value.is_some() || site.at_target.is_some();
                if site.has_constant_selector == has_at {
                    return None;
                }
                if site.has_constant_selector {
                    let mut spec = signature(
                        "Z",
                        vec![
                            parameter("obj", "Ljava/lang/Object;"),
                            operation_parameter("Z"),
                        ],
                    );
                    spec.operation_call_args = vec!["obj".into()];
                    spec.optional_captured_target_parameters =
                        parameters(&parameter_descriptors(&method.descriptor));
                    return Some(spec);
                }
                if at.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION") {
                    let layout = Expression::infer_simple_operation_layout(
                        source,
                        site,
                        &method,
                        targets,
                        self.bytecode_index?,
                        self.class_index,
                        context,
                    )?;
                    let mut params: Vec<_> = layout
                        .argument_descriptors
                        .iter()
                        .enumerate()
                        .map(|(i, d)| {
                            let mut p = parameter(layout.parameter_names[i].clone(), d);
                            if layout.argument_int_like.get(i) == Some(&true) {
                                p.accepted_type_descriptors = int_like_descriptors();
                            }
                            p
                        })
                        .collect();
                    let mut operation = operation_parameter(&layout.return_descriptor);
                    if layout.return_int_like {
                        operation.accepted_operation_generic_descriptors = int_like_descriptors();
                    }
                    params.push(operation);
                    let mut spec = signature(&layout.return_descriptor, params);
                    spec.operation_call_args = layout.parameter_names;
                    spec.optional_captured_target_parameters =
                        parameters(&parameter_descriptors(&method.descriptor));
                    if layout.return_int_like {
                        spec.accepted_return_type_descriptors = int_like_descriptors();
                    }
                    return Some(spec);
                }
                if at == "NEW" {
                    return self.expected_new(site, &method, targets);
                }
                if at == "FIELD" {
                    return Some(wrap_signature(
                        &field_wrapped(self.parse_field(
                            site.at_target.as_deref(),
                            &method,
                            targets,
                            field_operations(),
                            site.at_ordinal,
                        )?),
                        Some(&method),
                    ));
                }
                if at != "INVOKE" {
                    return None;
                }
                let wrapped = self.operation_invoke(
                    site.at_target.as_deref(),
                    &method,
                    targets,
                    site.at_ordinal,
                )?;
                if site.at_ordinal.is_some() {
                    self.validate_invoke_ordinal(site, &method, targets, wrapped.is_static)?;
                }
                Some(wrap_signature(&wrapped, Some(&method)))
            }
            WrapWithCondition => {
                let (status, wrapped) =
                    self.condition_target(source, site, &method, targets, context);
                if !matches!(
                    status,
                    WrapWithConditionTargetStatus::ValidVoid
                        | WrapWithConditionTargetStatus::ValidPoppedNonVoid
                ) {
                    return None;
                }
                let wrapped = wrapped?;
                let mut spec = signature("Z", wrapped_parameters(&wrapped));
                spec.optional_captured_target_parameters =
                    parameters(&parameter_descriptors(&method.descriptor));
                Some(spec)
            }
            _ => None,
        }
    }
    fn known_method_target(&self, target: &WrappedTarget) -> bool {
        let descriptor = format!("({}){}", target.parameters.concat(), target.return_type);
        self.class_index
            .get_methods(&target.owner)
            .iter()
            .any(|m| m.descriptor == descriptor)
    }
    pub fn wrap_with_condition_target_status(
        &self,
        site: &MixinExtrasAnnotationSite,
        targets: &[String],
        source: Option<&str>,
        context: Option<&ExpressionContext>,
    ) -> WrapWithConditionTargetStatus {
        let Some(method) = self.resolve_target_method(targets, &site.method_attribute) else {
            return WrapWithConditionTargetStatus::Unresolved;
        };
        if site
            .at_value
            .as_deref()
            .is_some_and(|at| at.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION"))
            && source.is_none()
        {
            return WrapWithConditionTargetStatus::Unresolved;
        }
        self.condition_target(source.unwrap_or(""), site, &method, targets, context)
            .0
    }
    fn condition_target(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        method: &MethodIndexEntry,
        targets: &[String],
        context: Option<&ExpressionContext>,
    ) -> (WrapWithConditionTargetStatus, Option<WrappedTarget>) {
        use WrapWithConditionTargetStatus::*;
        let Some(at) = site.at_value.as_ref() else {
            return (Unresolved, None);
        };
        let at = at.to_uppercase();
        if !["INVOKE", "FIELD", "MIXINEXTRAS:EXPRESSION"].contains(&at.as_str()) {
            return (InvalidInstruction, None);
        }
        let Some(index) = self.bytecode_index else {
            return (Unresolved, None);
        };
        if at == "MIXINEXTRAS:EXPRESSION" {
            use crate::mixinextras_expression::{
                ExpressionContextResolver, WrapWithConditionTargetKind,
            };
            let Some(layouts) = ExpressionContextResolver::infer_wrap_with_condition_layouts(
                source,
                site,
                method,
                targets,
                index,
                self.class_index,
                context,
            ) else {
                return (Unresolved, None);
            };
            if layouts.is_empty() {
                return (Unresolved, None);
            }
            let mut target = None;
            let mut popped = false;
            for layout in layouts {
                match layout.target_kind {
                    WrapWithConditionTargetKind::Field => {
                        if layout.result_classification != OccurrenceResultClassification::Void {
                            return (InvalidInstruction, None);
                        }
                    }
                    WrapWithConditionTargetKind::Invoke => match layout.result_classification {
                        OccurrenceResultClassification::Retained => {
                            return (InvalidRetainedNonVoid, None)
                        }
                        OccurrenceResultClassification::NotApplicable => {
                            return (InvalidInstruction, None)
                        }
                        OccurrenceResultClassification::ImmediatelyPopped => popped = true,
                        OccurrenceResultClassification::Void => {}
                    },
                    WrapWithConditionTargetKind::Other => return (InvalidInstruction, None),
                }
                let current = WrappedTarget {
                    owner: layout.owner_internal_name,
                    parameters: layout.parameter_descriptors,
                    is_static: layout.is_static,
                    return_type: "V".into(),
                };
                if target.as_ref().is_some_and(|previous| previous != &current) {
                    return (Unresolved, None);
                }
                target = Some(current);
            }
            return (
                if popped {
                    ValidPoppedNonVoid
                } else {
                    ValidVoid
                },
                target,
            );
        }
        let owners = self.owners(targets);
        if owners.is_empty() {
            return (Unresolved, None);
        }
        let Some(value) = site.at_target.as_deref().filter(|s| !s.trim().is_empty()) else {
            return (Unresolved, None);
        };
        let Ok(member) = descriptor::parse_member_target(value) else {
            return (Unresolved, None);
        };
        let (owner, name, full, params) = match member {
            MemberTarget::Method {
                owner,
                name,
                descriptor,
            } if at == "INVOKE" => {
                if name == "<init>" {
                    return (InvalidInstruction, None);
                }
                let full = descriptor::method_to_descriptor(&descriptor);
                (
                    owner,
                    name,
                    full,
                    descriptor
                        .parameters
                        .iter()
                        .map(descriptor::to_descriptor)
                        .collect(),
                )
            }
            MemberTarget::Field {
                owner,
                name,
                descriptor,
            } if at == "FIELD" => {
                let d = descriptor::to_descriptor(&descriptor);
                (owner, name, d.clone(), vec![d])
            }
            _ => return (InvalidInstruction, None),
        };
        let mut unified_static = None;
        let mut popped = false;
        for mixin_owner in owners {
            let candidates = index
                .get_at_target_candidates(&mixin_owner, &method.name, Some(&method.descriptor), &at)
                .into_iter()
                .filter(|c| {
                    c.owner == owner
                        && c.name == name
                        && c.descriptor == full
                        && if at == "INVOKE" {
                            c.kind == AtTargetKind::Invoke
                                && c.operation_kind.is_some_and(is_invoke)
                        } else {
                            c.kind == AtTargetKind::Field
                        }
                })
                .collect();
            let Some(selected) = select_candidates(candidates, site.at_ordinal) else {
                return (Unresolved, None);
            };
            if selected.is_empty() {
                return (Unresolved, None);
            }
            let mut owner_static = None;
            for candidate in selected {
                let is_static = if at == "INVOKE" {
                    match candidate.occurrence_result_classification {
                        OccurrenceResultClassification::Retained => {
                            return (InvalidRetainedNonVoid, None)
                        }
                        OccurrenceResultClassification::NotApplicable => {
                            return (InvalidInstruction, None)
                        }
                        OccurrenceResultClassification::ImmediatelyPopped => popped = true,
                        OccurrenceResultClassification::Void => {}
                    }
                    candidate.operation_kind == Some(AtTargetOperationKind::InvokeStatic)
                } else {
                    match candidate.operation_kind {
                        Some(
                            AtTargetOperationKind::FieldGetInstance
                            | AtTargetOperationKind::FieldGetStatic,
                        ) => return (InvalidInstruction, None),
                        Some(AtTargetOperationKind::FieldPutInstance) => false,
                        Some(AtTargetOperationKind::FieldPutStatic) => true,
                        _ => return (Unresolved, None),
                    }
                };
                if owner_static.is_some_and(|s| s != is_static) {
                    return (Unresolved, None);
                }
                owner_static = Some(is_static);
            }
            if unified_static.is_some_and(|s| Some(s) != owner_static) {
                return (Unresolved, None);
            }
            unified_static = owner_static;
        }
        let Some(is_static) = unified_static else {
            return (Unresolved, None);
        };
        (
            if popped {
                ValidPoppedNonVoid
            } else {
                ValidVoid
            },
            Some(WrappedTarget {
                owner,
                parameters: params,
                return_type: "V".into(),
                is_static,
            }),
        )
    }
    fn validate_invoke_ordinal(
        &self,
        site: &MixinExtrasAnnotationSite,
        method: &MethodIndexEntry,
        targets: &[String],
        is_static: bool,
    ) -> Option<()> {
        let MemberTarget::Method {
            owner,
            name,
            descriptor,
        } = descriptor::parse_member_target(site.at_target.as_deref()?).ok()?
        else {
            return None;
        };
        let full = descriptor::method_to_descriptor(&descriptor);
        let bytecode = self.bytecode_index?;
        let owners = self.owners(targets);
        if owners.is_empty() {
            return None;
        }
        for mixin_owner in owners {
            let candidates = bytecode
                .get_at_target_candidates(
                    &mixin_owner,
                    &method.name,
                    Some(&method.descriptor),
                    "INVOKE",
                )
                .into_iter()
                .filter(|c| {
                    c.kind == AtTargetKind::Invoke
                        && c.owner == owner
                        && c.name == name
                        && c.descriptor == full
                        && c.operation_kind.is_some_and(is_invoke)
                })
                .collect();
            let selected = select_candidates(candidates, site.at_ordinal)?
                .into_iter()
                .next()?;
            if (selected.operation_kind == Some(AtTargetOperationKind::InvokeStatic)) != is_static {
                return None;
            }
        }
        Some(())
    }
    pub fn infer_expression_value_type(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        method: &MethodIndexEntry,
        targets: &[String],
        context: Option<&ExpressionContext>,
    ) -> Option<String> {
        let at = site.at_value.as_ref()?.to_uppercase();
        match at.as_str(){"MIXINEXTRAS:EXPRESSION"=>crate::mixinextras_expression::ExpressionContextResolver::infer_expression_value_type(source,site,method,targets,self.bytecode_index?,self.class_index,context),"CONSTANT"=>crate::mixinextras_occurrence::ConstantAtArgsParser::parse(&site.at_args),"INVOKE"=>{let wrapped=self.parse_invoke(site.at_target.as_deref())?;(wrapped.return_type!="V").then_some(wrapped.return_type)},"FIELD"=>Some(self.parse_field(site.at_target.as_deref(),method,targets,&[AtTargetOperationKind::FieldGetInstance,AtTargetOperationKind::FieldGetStatic],None)?.descriptor),"NEW"=>{if let Some(target)=site.at_target.as_deref().filter(|s|!s.trim().is_empty()){let(owner,_)=parse_new_filter(target)?;return Some(format!("L{owner};"));}let bytecode=self.bytecode_index?;let owners=self.owners(targets);if owners.is_empty(){return None;}let descriptors:BTreeSet<_>=owners.into_iter().flat_map(|owner|bytecode.get_at_target_candidates(&owner,&method.name,Some(&method.descriptor),"NEW")).filter(|c|c.kind==AtTargetKind::New).filter_map(|c|parse_new_filter(&format!("L{};",c.owner)).map(|(owner,_)|format!("L{owner};"))).collect();(descriptors.len()==1).then(||descriptors.into_iter().next().unwrap())},_=>None}
    }
    fn parse_invoke(&self, target: Option<&str>) -> Option<WrappedTarget> {
        let MemberTarget::Method {
            owner,
            name,
            descriptor,
        } = descriptor::parse_member_target(target?).ok()?
        else {
            return None;
        };
        let full = descriptor::method_to_descriptor(&descriptor);
        let is_static = self
            .class_index
            .get_methods(&owner)
            .iter()
            .find(|m| m.name == name && m.descriptor == full)
            .is_some_and(|m| m.is_static);
        Some(WrappedTarget {
            owner,
            parameters: descriptor
                .parameters
                .iter()
                .map(descriptor::to_descriptor)
                .collect(),
            return_type: descriptor::to_descriptor(&descriptor.return_type),
            is_static,
        })
    }
    fn operation_invoke(
        &self,
        target: Option<&str>,
        method: &MethodIndexEntry,
        targets: &[String],
        ordinal: Option<i32>,
    ) -> Option<WrappedTarget> {
        let MemberTarget::Method {
            owner,
            name,
            descriptor,
        } = descriptor::parse_member_target(target?).ok()?
        else {
            return None;
        };
        if name == "<init>" {
            return None;
        }
        let full = descriptor::method_to_descriptor(&descriptor);
        let fallback = self.parse_invoke(target)?;
        let Some(index) = self.bytecode_index else {
            return Some(fallback);
        };
        let owners = self.owners(targets);
        if owners.is_empty() {
            return Some(fallback);
        }
        let mut receivers = BTreeSet::new();
        let mut is_static = None;
        for mixin_owner in owners {
            let candidates = index
                .get_at_target_candidates(
                    &mixin_owner,
                    &method.name,
                    Some(&method.descriptor),
                    "INVOKE",
                )
                .into_iter()
                .filter(|c| {
                    c.kind == AtTargetKind::Invoke
                        && c.owner == owner
                        && c.name == name
                        && c.descriptor == full
                })
                .collect();
            let candidates = select_candidates(candidates, ordinal)?;
            if candidates.is_empty() {
                return Some(fallback);
            }
            let mut kinds = vec![];
            for candidate in candidates {
                if let Some(k) = candidate.operation_kind {
                    if !kinds.contains(&k) {
                        kinds.push(k);
                    }
                }
            }
            if kinds.is_empty() {
                return Some(fallback);
            }
            if kinds.len() != 1 {
                return None;
            }
            let kind = kinds[0];
            let candidate_static = kind == AtTargetOperationKind::InvokeStatic;
            if is_static.is_some_and(|s| s != candidate_static) {
                return None;
            }
            is_static = Some(candidate_static);
            if !candidate_static {
                receivers.insert(match kind {
                    AtTargetOperationKind::InvokeSpecial => mixin_owner,
                    AtTargetOperationKind::InvokeVirtual
                    | AtTargetOperationKind::InvokeInterface => owner.clone(),
                    _ => return None,
                });
            }
        }
        if is_static == Some(false) && receivers.len() != 1 {
            return None;
        }
        Some(WrappedTarget {
            owner: receivers.into_iter().next().unwrap_or(owner),
            is_static: is_static.unwrap_or(fallback.is_static),
            ..fallback
        })
    }
    fn parse_field(
        &self,
        target: Option<&str>,
        method: &MethodIndexEntry,
        targets: &[String],
        allowed: &[AtTargetOperationKind],
        ordinal: Option<i32>,
    ) -> Option<FieldTarget> {
        let MemberTarget::Field {
            owner,
            name,
            descriptor,
        } = descriptor::parse_member_target(target?).ok()?
        else {
            return None;
        };
        let descriptor = descriptor::to_descriptor(&descriptor);
        let index = self.bytecode_index?;
        let owners = self.owners(targets);
        if owners.is_empty() {
            return None;
        }
        let mut kinds = vec![];
        for mixin_owner in owners {
            let candidates = index
                .get_at_target_candidates(
                    &mixin_owner,
                    &method.name,
                    Some(&method.descriptor),
                    "FIELD",
                )
                .into_iter()
                .filter(|c| {
                    c.kind == AtTargetKind::Field
                        && c.owner == owner
                        && c.name == name
                        && c.descriptor == descriptor
                })
                .collect();
            for c in select_candidates(candidates, ordinal)? {
                if ordinal.is_some() && c.operation_kind.is_none() {
                    return None;
                }
                if let Some(kind) = c.operation_kind {
                    if !kinds.contains(&kind) {
                        kinds.push(kind);
                    }
                }
            }
        }
        if kinds.len() != 1 || !allowed.contains(&kinds[0]) {
            return None;
        }
        Some(FieldTarget {
            owner,
            descriptor,
            operation: kinds[0],
        })
    }
    fn modify_receiver_target(
        &self,
        target: Option<&str>,
        method: &MethodIndexEntry,
        targets: &[String],
    ) -> Option<WrappedTarget> {
        match descriptor::parse_member_target(target?).ok()? {
            MemberTarget::Field { .. } => {
                let field = self.parse_field(
                    target,
                    method,
                    targets,
                    &[
                        AtTargetOperationKind::FieldGetInstance,
                        AtTargetOperationKind::FieldPutInstance,
                    ],
                    None,
                )?;
                Some(WrappedTarget {
                    owner: field.owner,
                    parameters: if field.operation == AtTargetOperationKind::FieldPutInstance {
                        vec![field.descriptor]
                    } else {
                        vec![]
                    },
                    return_type: "V".into(),
                    is_static: false,
                })
            }
            MemberTarget::Method {
                owner,
                name,
                descriptor,
            } => {
                let full = descriptor::method_to_descriptor(&descriptor);
                if self
                    .class_index
                    .get_methods(&owner)
                    .iter()
                    .any(|m| m.name == name && m.descriptor == full && m.is_static)
                {
                    return None;
                }
                let index = self.bytecode_index?;
                let owners = self.owners(targets);
                if owners.is_empty() {
                    return None;
                }
                let mut receivers = BTreeSet::new();
                for mixin_owner in owners {
                    let mut kinds = vec![];
                    for candidate in index
                        .get_at_target_candidates(
                            &mixin_owner,
                            &method.name,
                            Some(&method.descriptor),
                            "INVOKE",
                        )
                        .into_iter()
                        .filter(|c| {
                            c.kind == AtTargetKind::Invoke
                                && c.owner == owner
                                && c.name == name
                                && c.descriptor == full
                        })
                    {
                        if let Some(kind) = candidate.operation_kind {
                            if !kinds.contains(&kind) {
                                kinds.push(kind);
                            }
                        }
                    }
                    if kinds.len() != 1 {
                        return None;
                    }
                    receivers.insert(match kinds[0] {
                        AtTargetOperationKind::InvokeVirtual
                        | AtTargetOperationKind::InvokeInterface => owner.clone(),
                        AtTargetOperationKind::InvokeSpecial => mixin_owner,
                        _ => return None,
                    });
                }
                if receivers.len() != 1 {
                    return None;
                }
                Some(WrappedTarget {
                    owner: receivers.into_iter().next()?,
                    parameters: descriptor
                        .parameters
                        .iter()
                        .map(descriptor::to_descriptor)
                        .collect(),
                    return_type: "V".into(),
                    is_static: false,
                })
            }
        }
    }
    fn expected_new(
        &self,
        site: &MixinExtrasAnnotationSite,
        method: &MethodIndexEntry,
        targets: &[String],
    ) -> Option<HandlerSignatureSpec> {
        let index = self.bytecode_index?;
        let filter = if let Some(value) = site.at_target.as_deref().filter(|s| !s.trim().is_empty())
        {
            Some(parse_new_filter(value)?)
        } else {
            None
        };
        let owners = self.owners(targets);
        if owners.is_empty() {
            return None;
        }
        let mut unified = None;
        for owner in owners {
            let bytes = index.get_class_bytes(&owner)?;
            let pairs = new_pairs(&bytes, method)?;
            let candidates = index
                .get_at_target_candidates(&owner, &method.name, Some(&method.descriptor), "NEW")
                .into_iter()
                .filter(|c| c.kind == AtTargetKind::New)
                .filter(|c| filter.as_ref().is_none_or(|(owner, _)| c.owner == *owner))
                .filter(|c| {
                    site.at_ordinal.is_none_or(|ordinal| {
                        i32::try_from(c.ordinal.unwrap_or(0)).ok() == Some(ordinal)
                    })
                })
                .filter(|c| {
                    filter
                        .as_ref()
                        .and_then(|(_, d)| d.as_ref())
                        .is_none_or(|d| {
                            pairs.iter().any(|p| {
                                p.owner == c.owner
                                    && Some(p.ordinal) == i32::try_from(c.ordinal.unwrap_or(0)).ok()
                                    && p.descriptor == *d
                            })
                        })
                })
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                return None;
            }
            if site
                .at_target
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
                && candidates
                    .iter()
                    .map(|c| &c.owner)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != 1
            {
                return None;
            }
            let mut selected = vec![];
            for c in candidates {
                selected.push(pairs.iter().find(|p| {
                    p.owner == c.owner
                        && Some(p.ordinal) == i32::try_from(c.ordinal.unwrap_or(0)).ok()
                })?);
            }
            if selected
                .iter()
                .map(|p| &p.parameters)
                .collect::<BTreeSet<_>>()
                .len()
                != 1
            {
                return None;
            }
            let first = selected[0];
            let wrapped = WrappedTarget {
                owner: first.owner.clone(),
                parameters: first.parameters.clone(),
                return_type: format!("L{};", first.owner),
                is_static: true,
            };
            if unified
                .as_ref()
                .is_some_and(|previous| previous != &wrapped)
            {
                return None;
            }
            unified = Some(wrapped);
        }
        Some(wrap_signature(&unified?, Some(method)))
    }
    fn expected_variable(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        method: &MethodIndexEntry,
        targets: &[String],
    ) -> Option<HandlerSignatureSpec> {
        let at = site.at_value.as_ref()?.to_uppercase();
        if !["LOAD", "STORE"].contains(&at.as_str()) || site.at_shift != AtShiftSpec::Before {
            return None;
        }
        let index =
            annotation_value(source, site, "index").and_then(|s| s.trim().parse::<i32>().ok());
        let ordinal =
            annotation_value(source, site, "ordinal").and_then(|s| s.trim().parse::<i32>().ok());
        if index.is_some_and(|n| n < 0) || ordinal.is_some_and(|n| n < 0) {
            return None;
        }
        let names: [Option<String>; 2] = [
            annotation_value(source, site, "name"),
            annotation_value(source, site, "names"),
        ];
        let names: Vec<_> = names
            .into_iter()
            .flatten()
            .flat_map(|s| {
                source_tokens(&s, false)
                    .into_iter()
                    .filter_map(|t| {
                        if let SourceTokenKind::String {
                            content_start,
                            content_end,
                            ..
                        } = t.kind
                        {
                            Some(s[content_start..content_end].to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let args_only = annotation_value(source, site, "argsOnly")
            .is_some_and(|v| v.eq_ignore_ascii_case("true"));
        let bytecode = self.bytecode_index?;
        let owners = self.owners(targets);
        if owners.is_empty() {
            return None;
        }
        let mut types = BTreeSet::new();
        for owner in owners {
            let bytes = bytecode.get_class_bytes(&owner)?;
            let flow = decode_handler_method(&bytes, method)?;
            let instructions: Vec<_> = flow
                .instructions
                .iter()
                .filter(|i| {
                    if at == "LOAD" {
                        matches!(i.opcode, 21..=25)
                    } else {
                        matches!(i.opcode, 54..=58)
                    }
                })
                .filter_map(|i| {
                    if let crate::expression_engine::InstructionOperand::Local(slot) = i.operand {
                        Some((slot as i32, i.original_index as i32))
                    } else {
                        None
                    }
                })
                .filter(|(slot, _)| index.is_none_or(|wanted| *slot == wanted))
                .collect();
            if instructions.is_empty() {
                return None;
            }
            let selected = if let Some(n) = site.at_ordinal {
                vec![*instructions.get(usize::try_from(n).ok()?)?]
            } else {
                instructions
            };
            let indices = selected.iter().map(|(_, i)| *i).collect();
            let captures = crate::mixinextras_locals::LocalCaptureExtractor::extract(
                Some(&bytes),
                &method.name,
                &method.descriptor,
                &indices,
            )
            .unwrap_or_else(|cancelled| std::panic::panic_any(cancelled));
            let LocalCaptureResult::Success { snapshots } = captures else {
                return None;
            };
            for (_, occurrence) in selected {
                let matches: Vec<_> = snapshots
                    .iter()
                    .filter(|s| s.instruction_occurrence_index == occurrence)
                    .collect();
                if matches.len() != 1 {
                    return None;
                }
                let candidates: Vec<_> = matches[0]
                    .candidates
                    .iter()
                    .filter(|c| {
                        index.is_none_or(|i| c.slot_index == i)
                            && (!args_only || c.is_argument)
                            && (names.is_empty()
                                || c.name.as_ref().is_some_and(|n| names.contains(n)))
                            && ordinal.is_none_or(|o| c.ordinal == Some(o))
                    })
                    .collect();
                if candidates.len() != 1 {
                    return None;
                }
                types.insert(candidates[0].descriptor.clone());
            }
        }
        if types.len() != 1 {
            return None;
        }
        Some(original_signature(&types.into_iter().next()?))
    }
    /// Compatibility predicate: true proves a match. Diagnostic consumers must
    /// use type_match_knowledge to distinguish incompatibility from missing facts.
    pub fn return_type_matches(
        &self,
        expected: &HandlerSignatureSpec,
        handler: &HandlerMethodDeclaration,
    ) -> bool {
        self.type_matches(
            handler.return_type_descriptor.as_deref(),
            &expected.return_type_descriptor,
            handler.has_method_level_coerce,
            &expected.accepted_return_type_descriptors,
        )
    }
    /// Compatibility predicate; false alone does not prove incompatibility.
    pub fn parameter_type_matches(
        &self,
        expected: &HandlerParameterSpec,
        actual: &HandlerParameterDeclaration,
    ) -> bool {
        if expected.is_operation {
            actual.is_operation
                && expected.operation_generic_descriptor.is_some()
                && operation_generic_matches(expected, actual)
        } else {
            self.type_matches(
                actual.type_descriptor.as_deref(),
                &expected.type_descriptor,
                actual.has_coerce,
                &expected.accepted_type_descriptors,
            )
        }
    }
    fn type_matches(
        &self,
        actual: Option<&str>,
        expected: &str,
        coerce: bool,
        accepted: &BTreeSet<String>,
    ) -> bool {
        self.type_match_knowledge(actual, expected, coerce, accepted) == Some(true)
    }
    pub fn known_reference(&self, name: &str) -> Option<bool> {
        if name == "java/lang/Object"
            || self.class_index.find_class(name).is_some()
            || self
                .bytecode_index
                .is_some_and(|b| b.get_class_bytes(name).is_some())
        {
            return Some(true);
        }
        self.reference_types
            .and_then(|r| r.is_known_reference(name))
    }
    pub fn coerce_reference_knowledge(&self, from: &str, expected: &str) -> Option<bool> {
        if from == expected {
            return Some(true);
        }
        if from == "java/lang/Object" {
            return self.known_reference(expected);
        }
        let from_descriptor = format!("L{from};");
        let expected_descriptor = format!("L{expected};");
        if self
            .bytecode_index
            .and_then(|b| b.resolve_common_super_class(&from_descriptor, &expected_descriptor))
            .as_deref()
            == Some(&from_descriptor)
        {
            return Some(true);
        }
        match (self.known_reference(from), self.known_reference(expected)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => self
                .reference_types
                .and_then(|r| r.is_assignable(from, expected)),
            _ => None,
        }
    }
    pub fn validate_common_sugar_constraints(
        &self,
        handler: &HandlerMethodDeclaration,
        target: Option<&MethodIndexEntry>,
        annotation: Option<MixinExtrasAnnotation>,
    ) -> Vec<HandlerValidationIssue> {
        let mut issues = vec![];
        let issue = |code: &str, message: String, range: McTextRange| HandlerValidationIssue {
            code: code.into(),
            message,
            range,
        };
        if let Some(last) = handler.parameters.iter().rposition(|p| !p.is_sugar) {
            if let Some(p) = handler.parameters[..last].iter().find(|p| p.is_sugar) {
                issues.push(issue(
                    diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                    "Sugar parameters must be trailing".into(),
                    p.sugar_annotation_range
                        .clone()
                        .or(p.range.clone())
                        .unwrap_or_else(|| handler.range.clone()),
                ));
            }
        }
        for p in &handler.parameters {
            if p.is_sugar
                && matches!(p.sugar_spec, Some(HandlerParameterSugarSpec::Share(_)))
                && p.type_descriptor
                    .as_ref()
                    .is_some_and(|d| Self::share_ref_value_descriptor(d).is_none())
            {
                issues.push(issue(
                    diagnostic_codes::INVALID_SHARE_PARAMETER_TYPE,
                    "Share parameter type must be in com.llamalad7.mixinextras.sugar.ref".into(),
                    handler.range.clone(),
                ));
            }
        }
        if let Some(target) = target {
            let expected = if return_descriptor(&target.descriptor) == "V" {
                CALLBACK_INFO
            } else {
                CALLBACK_INFO_RETURNABLE
            };
            for p in &handler.parameters {
                if p.is_sugar
                    && p.sugar_spec == Some(HandlerParameterSugarSpec::Cancellable)
                    && p.type_descriptor.as_ref().is_some_and(|d| d != expected)
                {
                    issues.push(issue(
                        diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                        format!(
                            "Cancellable parameter type should be {}",
                            OperationSignatureRenderer::readable_type(expected)
                        ),
                        p.sugar_annotation_range
                            .clone()
                            .unwrap_or_else(|| handler.range.clone()),
                    ));
                }
            }
            if annotation == Some(MixinExtrasAnnotation::Inject)
                && handler
                    .parameters
                    .iter()
                    .any(|p| p.sugar_spec == Some(HandlerParameterSugarSpec::Cancellable))
            {
                let ordinary: Vec<_> = handler.parameters.iter().filter(|p| !p.is_sugar).collect();
                let count = parameter_descriptors(&target.descriptor).len();
                let callback = ordinary
                    .get(count)
                    .is_some_and(|p| p.type_descriptor.as_deref() == Some(expected))
                    || (ordinary.len() == 1
                        && ordinary[0].type_descriptor.as_deref() == Some(expected));
                if !callback {
                    let sugar = handler
                        .parameters
                        .iter()
                        .find(|p| p.sugar_spec == Some(HandlerParameterSugarSpec::Cancellable))
                        .unwrap();
                    issues.push(issue(diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,format!("Inject handler must include {} as its ordinary callback parameter when using @Cancellable",OperationSignatureRenderer::readable_type(expected)),sugar.sugar_annotation_range.clone().or(sugar.range.clone()).unwrap_or_else(||handler.range.clone())));
                }
            }
        }
        issues
    }
    pub fn specialize_int_like_signature(
        &self,
        expected: &HandlerSignatureSpec,
        handler: &HandlerMethodDeclaration,
    ) -> HandlerSignatureSpec {
        let base: Vec<_> = handler.parameters.iter().filter(|p| !p.is_sugar).collect();
        let mut choices = BTreeSet::new();
        if !expected.accepted_return_type_descriptors.is_empty() {
            if let Some(actual) = handler
                .return_type_descriptor
                .as_ref()
                .filter(|d| expected.accepted_return_type_descriptors.contains(*d))
            {
                choices.insert(actual.clone());
            }
        }
        for (i, p) in expected.parameters.iter().enumerate() {
            if !p.accepted_type_descriptors.is_empty() {
                if let Some(actual) = base
                    .get(i)
                    .and_then(|a| a.type_descriptor.as_ref())
                    .filter(|d| p.accepted_type_descriptors.contains(*d))
                {
                    choices.insert(actual.clone());
                }
            }
        }
        let choice = if choices.len() == 1 {
            choices.into_iter().next()
        } else {
            let choices: BTreeSet<_> = expected
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.accepted_operation_generic_descriptors.is_empty())
                .filter_map(|(i, p)| {
                    base.get(i)
                        .filter(|a| a.is_operation)
                        .and_then(|a| operation_generic_descriptor(p, a))
                })
                .collect();
            (choices.len() == 1).then(|| choices.into_iter().next().unwrap())
        };
        let Some(choice) = choice else {
            return expected.clone();
        };
        let mut result = expected.clone();
        if !result.accepted_return_type_descriptors.is_empty() {
            result.return_type_descriptor = choice.clone();
        }
        result.readable_return_type =
            OperationSignatureRenderer::readable_type(&result.return_type_descriptor);
        for p in &mut result.parameters {
            if !p.accepted_type_descriptors.is_empty() {
                p.type_descriptor = choice.clone();
                p.readable_type = OperationSignatureRenderer::readable_type(&choice);
            } else if !p.accepted_operation_generic_descriptors.is_empty() {
                p.operation_generic_descriptor = Some(choice.clone());
                p.readable_type = OperationSignatureRenderer::render_operation_type(&choice);
            }
        }
        result
    }
}
fn return_descriptor(descriptor: &str) -> &str {
    descriptor.split_once(')').map_or("V", |(_, r)| r)
}
fn select_candidates(
    mut candidates: Vec<AtTargetCandidate>,
    ordinal: Option<i32>,
) -> Option<Vec<AtTargetCandidate>> {
    if let Some(ordinal) = ordinal {
        if ordinal < 0
            || candidates
                .iter()
                .any(|c| c.instruction_occurrence_index < 0)
        {
            return None;
        }
        candidates.sort_by_key(|c| c.instruction_occurrence_index);
        Some(vec![candidates.get(ordinal as usize)?.clone()])
    } else {
        Some(candidates)
    }
}
fn parameter(name: impl Into<String>, descriptor: &str) -> HandlerParameterSpec {
    HandlerParameterSpec {
        name: name.into(),
        type_descriptor: descriptor.into(),
        readable_type: OperationSignatureRenderer::readable_type(descriptor),
        ..Default::default()
    }
}
fn parameters(descriptors: &[String]) -> Vec<HandlerParameterSpec> {
    descriptors
        .iter()
        .enumerate()
        .map(|(i, d)| parameter(format!("arg{i}"), d))
        .collect()
}
fn operation_parameter(descriptor: &str) -> HandlerParameterSpec {
    HandlerParameterSpec {
        name: "original".into(),
        type_descriptor: format!("L{};", OPERATION_FQN.replace('.', "/")),
        readable_type: OperationSignatureRenderer::render_operation_type(descriptor),
        is_operation: true,
        operation_generic_descriptor: Some(descriptor.into()),
        ..Default::default()
    }
}
fn signature(return_type: &str, parameters: Vec<HandlerParameterSpec>) -> HandlerSignatureSpec {
    HandlerSignatureSpec {
        return_type_descriptor: return_type.into(),
        readable_return_type: OperationSignatureRenderer::readable_type(return_type),
        parameters,
        ..Default::default()
    }
}
fn original_signature(descriptor: &str) -> HandlerSignatureSpec {
    signature(descriptor, vec![parameter("original", descriptor)])
}
fn receiver_name(owner: &str) -> String {
    if owner.starts_with("java/") {
        return "instance".into();
    }
    let simple = owner.rsplit('/').next().unwrap_or(owner);
    let mut chars = simple.chars();
    chars
        .next()
        .map(|c| c.to_lowercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}
fn wrapped_parameters(wrapped: &WrappedTarget) -> Vec<HandlerParameterSpec> {
    let mut result = vec![];
    if !wrapped.is_static {
        result.push(parameter(
            receiver_name(&wrapped.owner),
            &format!("L{};", wrapped.owner),
        ));
    }
    result.extend(parameters(&wrapped.parameters));
    result
}
fn wrap_signature(
    wrapped: &WrappedTarget,
    method: Option<&MethodIndexEntry>,
) -> HandlerSignatureSpec {
    let mut params = wrapped_parameters(wrapped);
    let args = params.iter().map(|p| p.name.clone()).collect();
    params.push(operation_parameter(&wrapped.return_type));
    let mut spec = signature(&wrapped.return_type, params);
    spec.operation_call_args = args;
    spec.optional_captured_target_parameters = method
        .map(|m| parameters(&parameter_descriptors(&m.descriptor)))
        .unwrap_or_default();
    spec
}
fn int_like_descriptors() -> BTreeSet<String> {
    ["I", "Z", "B", "C", "S"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
fn field_operations() -> &'static [AtTargetOperationKind] {
    &[
        AtTargetOperationKind::FieldGetInstance,
        AtTargetOperationKind::FieldGetStatic,
        AtTargetOperationKind::FieldPutInstance,
        AtTargetOperationKind::FieldPutStatic,
    ]
}
fn field_wrapped(field: FieldTarget) -> WrappedTarget {
    let put = matches!(
        field.operation,
        AtTargetOperationKind::FieldPutInstance | AtTargetOperationKind::FieldPutStatic
    );
    WrappedTarget {
        owner: field.owner,
        parameters: if put {
            vec![field.descriptor.clone()]
        } else {
            vec![]
        },
        return_type: if put { "V".into() } else { field.descriptor },
        is_static: matches!(
            field.operation,
            AtTargetOperationKind::FieldGetStatic | AtTargetOperationKind::FieldPutStatic
        ),
    }
}
fn is_invoke(kind: AtTargetOperationKind) -> bool {
    matches!(
        kind,
        AtTargetOperationKind::InvokeVirtual
            | AtTargetOperationKind::InvokeStatic
            | AtTargetOperationKind::InvokeSpecial
            | AtTargetOperationKind::InvokeInterface
    )
}
fn annotation_value(source: &str, site: &MixinExtrasAnnotationSite, name: &str) -> Option<String> {
    let start = position_to_offset(source, site.annotation_range.start.clone())?;
    let end = position_to_offset(source, site.annotation_range.end.clone())?;
    let annotation = source.get(start..end)?;
    let open = annotation.find('(')?;
    let close = annotation.rfind(')')?;
    attributes(&annotation[open + 1..close])
        .into_iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.into())
}
fn parse_new_filter(value: &str) -> Option<(String, Option<String>)> {
    use descriptor::{JvmType, Pattern};
    if value.is_empty()
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
        || !value.starts_with('L')
        || value.find(';')? > value.find('(').unwrap_or(usize::MAX)
    {
        return None;
    }
    let selector = descriptor::parse_method_selector(value).ok()?;
    let Pattern::Exact(owner) = selector.owner else {
        return None;
    };
    let descriptor = match (selector.name, selector.descriptor) {
        (Pattern::Any, Pattern::Any) => None,
        (Pattern::Exact(name), Pattern::Exact(d))
            if name == "<init>" && d.return_type == JvmType::VoidType =>
        {
            Some(descriptor::method_to_descriptor(&d))
        }
        _ => return None,
    };
    Some((owner, descriptor))
}
fn decode_handler_method(
    bytes: &[u8],
    method: &MethodIndexEntry,
) -> Option<crate::expression_engine::MethodFlow> {
    use crate::expression_engine::*;
    let cancellation = current_cancellation();
    let pool = IdentifierPool::default();
    let common = |left: &str, right: &str| (left == right).then(|| left.to_string());
    let context = MatchContext {
        context_type: MatchContextType::Custom,
        identifier_pool: &pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &common,
        cancellation: cancellation.as_ref(),
    };
    match decode_method(bytes, &method.name, &method.descriptor, &context) {
        Ok(result) => result.ok(),
        Err(cancelled) => std::panic::panic_any(cancelled),
    }
}
struct NewPair {
    owner: String,
    ordinal: i32,
    descriptor: String,
    parameters: Vec<String>,
}
fn new_pairs(bytes: &[u8], method: &MethodIndexEntry) -> Option<Vec<NewPair>> {
    use crate::expression_engine::InstructionOperand;
    let flow = decode_handler_method(bytes, method)?;
    let mut owners = vec![];
    let mut constructors = BTreeMap::new();
    let mut pending = vec![];
    let mut initialized = method.name != "<init>";
    for instruction in flow.instructions {
        if instruction.opcode == 187 {
            if !initialized {
                return None;
            }
            let InstructionOperand::Type(owner) = instruction.operand else {
                return None;
            };
            pending.push(owners.len());
            owners.push(owner);
        } else if instruction.opcode == 183 {
            if let InstructionOperand::Member {
                owner,
                name,
                descriptor,
                ..
            } = instruction.operand
            {
                if name != "<init>" {
                    continue;
                }
                if let Some(index) = pending.pop() {
                    if owners[index] != owner || constructors.insert(index, descriptor).is_some() {
                        return None;
                    }
                } else if !initialized {
                    initialized = true;
                }
            }
        }
    }
    if !pending.is_empty() || constructors.len() != owners.len() {
        return None;
    }
    let mut ordinals = BTreeMap::new();
    owners
        .into_iter()
        .enumerate()
        .map(|(i, owner)| {
            let ordinal = ordinals.entry(owner.clone()).or_insert(0);
            let current = *ordinal;
            *ordinal += 1;
            let descriptor = constructors.remove(&i)?;
            Some(NewPair {
                owner,
                ordinal: current,
                parameters: parameter_descriptors(&descriptor),
                descriptor,
            })
        })
        .collect()
}
fn parameter_descriptors(descriptor: &str) -> Vec<String> {
    crate::descriptor::parse_method_descriptor(descriptor)
        .map(|m| {
            m.parameters
                .iter()
                .map(crate::descriptor::to_descriptor)
                .collect()
        })
        .unwrap_or_default()
}
fn operation_generic_matches(
    expected: &HandlerParameterSpec,
    actual: &HandlerParameterDeclaration,
) -> bool {
    expected
        .operation_generic_descriptor
        .as_deref()
        .is_some_and(|d| operation_generic_match(d, actual))
        || expected
            .accepted_operation_generic_descriptors
            .iter()
            .any(|d| operation_generic_match(d, actual))
}
fn operation_generic_descriptor(
    expected: &HandlerParameterSpec,
    actual: &HandlerParameterDeclaration,
) -> Option<String> {
    expected
        .operation_generic_descriptor
        .iter()
        .chain(expected.accepted_operation_generic_descriptors.iter())
        .find(|d| operation_generic_match(d, actual))
        .cloned()
}
fn sugar_is_trailing(parameters: &[HandlerParameterDeclaration]) -> bool {
    let mut sugar = false;
    for p in parameters {
        if p.is_sugar {
            sugar = true;
        } else if sugar {
            return false;
        }
    }
    true
}
fn int_like_types_match(
    expected: &HandlerSignatureSpec,
    handler: &HandlerMethodDeclaration,
) -> bool {
    let base: Vec<_> = handler.parameters.iter().filter(|p| !p.is_sugar).collect();
    let mut types = BTreeSet::new();
    if !expected.accepted_return_type_descriptors.is_empty() {
        if let Some(actual) = &handler.return_type_descriptor {
            if !expected.accepted_return_type_descriptors.contains(actual) {
                return false;
            }
            types.insert(actual.clone());
        }
    }
    for (i, parameter) in expected.parameters.iter().enumerate() {
        if !parameter.accepted_type_descriptors.is_empty() {
            let Some(actual) = base.get(i).and_then(|p| p.type_descriptor.as_ref()) else {
                return false;
            };
            if !parameter.accepted_type_descriptors.contains(actual) {
                return false;
            }
            types.insert(actual.clone());
        }
        if !parameter.accepted_operation_generic_descriptors.is_empty() {
            let Some(actual) = base.get(i).filter(|p| p.is_operation) else {
                return false;
            };
            let Some(descriptor) = operation_generic_descriptor(parameter, actual) else {
                return false;
            };
            types.insert(descriptor);
        }
    }
    types.len() <= 1
}
fn operation_generic_match(expected: &str, actual: &HandlerParameterDeclaration) -> bool {
    if let Some(descriptor) = &actual.operation_generic_descriptor {
        return descriptor == expected
            || boxed_generic(expected)
                .is_some_and(|name| descriptor == &format!("Ljava/lang/{name};"));
    }
    actual.operation_generic_name.as_ref().is_some_and(|name| {
        name.eq_ignore_ascii_case(&OperationSignatureRenderer::readable_type(expected))
            || boxed_generic(expected).is_some_and(|boxed| name.eq_ignore_ascii_case(boxed))
    })
}
#[derive(Default)]
struct AtInfo {
    value: Option<String>,
    target: Option<String>,
    args: Vec<String>,
    id: Option<String>,
    ordinal: Option<i32>,
    shift: AtShiftSpec,
    zero: BTreeSet<i32>,
}
fn find_sites(source: &str, sugar: bool) -> Vec<MixinExtrasAnnotationSite> {
    let imports = JavaTypeDescriptorResolver::imports_for(source);
    let annotations = annotations_with_imports(source, &imports);
    let tokens = source_tokens(source, imports.is_kotlin);
    let mut result = vec![];
    for a in &annotations {
        let Some(annotation) = MixinExtrasAnnotation::from_mixin_annotation(a.annotation) else {
            continue;
        };
        if !(if sugar {
            annotation.is_sugar_handler_injector()
        } else {
            annotation.is_injector()
        }) {
            continue;
        }
        // The completion parser also exposes incomplete annotations up to EOF.
        // Handler extraction requires a complete parent annotation, as the
        // legacy matching-paren gate did, before interpreting its attributes.
        let Some(open) = tokens
            .iter()
            .position(|t| t.text == "(" && t.end == a.body_start)
        else {
            continue;
        };
        if matching(&tokens, open).is_none() {
            continue;
        }
        let body = &source[a.body_start..a.body_end];
        let attrs = attributes(body);
        let methods = attrs
            .iter()
            .find(|(k, _)| *k == "method")
            .map(|(_, v)| string_values(v, imports.is_kotlin))
            .unwrap_or_default();
        if methods.is_empty() {
            continue;
        }
        let mut block = a.start;
        for prev in annotations.iter().rev() {
            if prev.end > block {
                continue;
            }
            let between = AnnotationContextExtractor::mask_non_code(&source[prev.end..block]);
            if between.trim().is_empty() {
                block = prev.start;
            } else {
                break;
            }
        }
        let handler = HandlerSignatureService::parse_handler_method(source, block, Some(&imports));
        let infos = if annotation == MixinExtrasAnnotation::ModifyConstant {
            constant_infos(body, &imports).unwrap_or_else(|| vec![AtInfo::default()])
        } else {
            extract_at_infos(body, &imports)
        };
        let has_constant = annotation == MixinExtrasAnnotation::WrapOperation
            && attrs
                .iter()
                .any(|(k, v)| *k == "constant" && v.trim_start().starts_with("@Constant"));
        for method in methods {
            for info in &infos {
                result.push(MixinExtrasAnnotationSite {
                    annotation,
                    method_attribute: method.clone(),
                    at_value: info.value.clone(),
                    at_target: info.target.clone(),
                    at_args: info.args.clone(),
                    at_id: info.id.clone(),
                    at_ordinal: info.ordinal,
                    at_shift: info.shift,
                    has_constant_selector: has_constant,
                    annotation_range: source_range(source, a.start, a.end),
                    handler_method: handler.clone(),
                    expand_zero_conditions: info.zero.clone(),
                });
            }
        }
    }
    result
}
fn trim_comments(value: &str) -> &str {
    let tokens = source_tokens(value, true);
    tokens.first().map(|t| &value[t.start..]).unwrap_or("")
}
fn extract_at_infos(body: &str, imports: &JavaSourceImports) -> Vec<AtInfo> {
    let attrs = attributes(body);
    let Some((_, value)) = attrs.iter().find(|(k, _)| *k == "at") else {
        return vec![AtInfo::default()];
    };
    let tokens = source_tokens(value, imports.is_kotlin);
    let mut result = vec![];
    for (i, t) in tokens.iter().enumerate() {
        if t.text != "(" {
            continue;
        }
        let Some(prev) = i.checked_sub(1) else {
            continue;
        };
        let mut begin = prev;
        while begin >= 2 && tokens[begin - 1].text == "." {
            begin -= 2;
        }
        let name = value[tokens[begin].start..tokens[prev].end].trim();
        if resolve_annotation(name, imports) != Some(MixinAnnotation::At)
            || (!imports.is_kotlin && (begin == 0 || tokens[begin - 1].text != "@"))
        {
            continue;
        }
        let Some(close) = matching(&tokens, i) else {
            result.push(AtInfo::default());
            continue;
        };
        result.push(parse_at_info(&value[t.end..tokens[close].start]));
    }
    if result.is_empty() {
        result.push(AtInfo::default());
    }
    result
}
fn at_string(value: &str) -> Option<String> {
    let value = trim_comments(value);
    let tokens = source_tokens(value, false);
    let t = tokens.first()?;
    if let SourceTokenKind::String {
        content_start,
        content_end,
        ..
    } = t.kind
    {
        let raw = &value[content_start..content_end];
        let mut result = String::new();
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                let next = chars.next()?;
                if next == '\\' || next == '"' {
                    result.push(next);
                } else {
                    result.push(c);
                    result.push(next);
                }
            } else {
                result.push(c);
            }
        }
        Some(result)
    } else {
        None
    }
}
fn at_integer(value: &str) -> Option<i32> {
    let value = trim_comments(value);
    let bytes = value.as_bytes();
    let mut i = usize::from(bytes.first().is_some_and(|b| *b == b'+' || *b == b'-'));
    let start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == start
        || bytes
            .get(i)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'.')
    {
        return None;
    }
    value[..i].parse().ok()
}
fn parse_at_info(body: &str) -> AtInfo {
    let mut info = AtInfo::default();
    let mut shift = None;
    let mut by = None;
    let mut bad_shift = false;
    for (a, b) in segments(body) {
        let text = trim_comments(&body[a..b]);
        let Some(eq) = source_tokens(text, true)
            .into_iter()
            .find(|t| t.text == "=")
            .map(|t| t.start)
        else {
            if info.value.is_none() {
                info.value = at_string(text);
            }
            continue;
        };
        let key = text[..eq].trim();
        let value = &text[eq + 1..];
        match key {
            "value" => info.value = at_string(value),
            "target" => info.target = at_string(value),
            "id" => info.id = at_string(value),
            "ordinal" => info.ordinal = at_integer(value).filter(|n| *n != -1),
            "args" => {
                let value = trim_comments(value);
                if value.starts_with(['{', '[']) {
                    let tokens = source_tokens(value, false);
                    let Some(end) = matching(&tokens, 0) else {
                        break;
                    };
                    let content = &value[1..tokens[end].start];
                    let mut args = vec![];
                    let mut valid = true;
                    for (a, b) in segments(content) {
                        let text = trim_comments(&content[a..b]);
                        if text.is_empty() && content.trim().is_empty() {
                            continue;
                        }
                        if let Some(s) = at_string(text) {
                            args.push(s);
                        } else {
                            valid = false;
                            break;
                        }
                    }
                    if !valid {
                        break;
                    }
                    info.args = args;
                } else if let Some(s) = at_string(value) {
                    info.args = vec![s];
                } else {
                    break;
                }
            }
            "shift" => {
                let value = trim_comments(value).trim();
                if value.split('.').all(|p| {
                    !p.is_empty()
                        && p.chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                }) {
                    shift = Some(value);
                } else {
                    bad_shift = true;
                }
            }
            "by" => by = Some(at_integer(value)),
            _ => {}
        }
    }
    info.shift = if bad_shift {
        AtShiftSpec::Unresolved
    } else if let Some(raw) = shift {
        let simple = raw.rsplit('.').next().unwrap();
        let valid = raw
            .rsplit_once('.')
            .is_none_or(|(p, _)| p == "At.Shift" || p.ends_with(".At.Shift"));
        if !valid {
            AtShiftSpec::Unresolved
        } else {
            match simple {
                "BEFORE" => AtShiftSpec::Before,
                "AFTER" => AtShiftSpec::After,
                "BY" => match by {
                    None => AtShiftSpec::By { offset: 0 },
                    Some(Some(offset)) => AtShiftSpec::By { offset },
                    _ => AtShiftSpec::Unresolved,
                },
                _ => AtShiftSpec::Unresolved,
            }
        }
    } else {
        AtShiftSpec::Before
    };
    info
}
fn constant_infos(body: &str, imports: &JavaSourceImports) -> Option<Vec<AtInfo>> {
    let attrs = attributes(body);
    if let Some((_, slice)) = attrs.iter().find(|(k, _)| *k == "slice") {
        let slice = slice.trim();
        if !(slice == if imports.is_kotlin { "[]" } else { "{}" }
            || annotation_body(slice, imports, MixinAnnotation::Slice)
                .is_some_and(|s| s.trim().is_empty()))
        {
            return None;
        }
    }
    let Some((_, value)) = attrs.iter().find(|(k, _)| *k == "constant") else {
        return Some(vec![AtInfo {
            value: Some("CONSTANT".into()),
            ..Default::default()
        }]);
    };
    let value = value.trim();
    let values = if value.starts_with('{') || (imports.is_kotlin && value.starts_with('[')) {
        let tokens = source_tokens(value, imports.is_kotlin);
        let close = matching(&tokens, 0)?;
        let body = &value[1..tokens[close].start];
        if body.trim().is_empty() {
            vec![""]
        } else {
            segments(body)
                .into_iter()
                .map(|(a, b)| &body[a..b])
                .collect()
        }
    } else {
        vec![value]
    };
    values
        .into_iter()
        .map(|value| {
            if value.is_empty() {
                return Some(AtInfo {
                    value: Some("CONSTANT".into()),
                    ..Default::default()
                });
            }
            parse_constant_info(
                annotation_body(value, imports, MixinAnnotation::Constant)?,
                imports,
            )
        })
        .collect()
}
fn annotation_body<'a>(
    value: &'a str,
    imports: &JavaSourceImports,
    kind: MixinAnnotation,
) -> Option<&'a str> {
    let tokens = source_tokens(value, imports.is_kotlin);
    let open = tokens.iter().position(|t| t.text == "(")?;
    let first = tokens.first()?;
    if !imports.is_kotlin && first.text != "@" {
        return None;
    }
    let start = usize::from(first.text == "@");
    let name = &value[tokens.get(start)?.start..tokens[open].start];
    let valid = if imports.is_kotlin {
        resolve_annotation(name.trim(), imports) == Some(kind)
    } else {
        name.trim().rsplit('.').next() == Some(kind.simple_name())
    };
    if !valid {
        return None;
    }
    let close = matching(&tokens, open)?;
    if close + 1 != tokens.len() {
        return None;
    }
    Some(&value[tokens[open].end..tokens[close].start])
}
fn parse_constant_info(body: &str, imports: &JavaSourceImports) -> Option<AtInfo> {
    let mut info = AtInfo {
        value: Some("CONSTANT".into()),
        ..Default::default()
    };
    for (a, b) in segments(body) {
        let text = body[a..b].trim();
        if text.is_empty() {
            continue;
        }
        let attrs = attributes(text);
        let (key, value) = *attrs.first()?;
        let normalized = match key {
            "slice" => {
                if !parsed_string(value, false)?.is_empty() {
                    return None;
                }
                continue;
            }
            "ordinal" => {
                let n = at_integer(value)?;
                if n < -1 {
                    return None;
                }
                info.ordinal = (n != -1).then_some(n);
                continue;
            }
            "log" => {
                if !value.eq_ignore_ascii_case("true") && !value.eq_ignore_ascii_case("false") {
                    return None;
                }
                continue;
            }
            "intValue" => at_integer(value)?.to_string(),
            "longValue" => value
                .trim_end_matches(['l', 'L'])
                .parse::<i64>()
                .ok()?
                .to_string(),
            "floatValue" | "doubleValue" => {
                let n = if key == "floatValue" {
                    crate::mixinextras_occurrence::parse_java_f32(value)? as f64
                } else {
                    crate::mixinextras_occurrence::parse_java_float(value)?
                };
                java_number(n, key == "floatValue")
            }
            "stringValue" => parsed_string(value, false)?,
            "classValue" => class_literal(value)?,
            "nullValue" => {
                if !value.eq_ignore_ascii_case("true") {
                    return None;
                }
                "true".into()
            }
            "expandZeroConditions" => {
                info.zero = zero_conditions(value, imports)?;
                continue;
            }
            _ => return None,
        };
        info.args.push(format!("{key}={normalized}"));
    }
    if !info.args.is_empty()
        && crate::mixinextras_occurrence::ConstantAtArgsParser::parse(&info.args).is_none()
    {
        return None;
    }
    Some(info)
}
fn java_number(n: f64, float: bool) -> String {
    let value = if float {
        crate::bytecode::ConstantValue::FloatValue(n as f32)
    } else {
        crate::bytecode::ConstantValue::DoubleValue(n)
    };
    let label = crate::bytecode::constant_label(Some(&value));
    if float {
        label.strip_suffix('f').unwrap_or(&label).to_string()
    } else {
        label
    }
}

fn zero_conditions(value: &str, imports: &JavaSourceImports) -> Option<BTreeSet<i32>> {
    let value = value.trim();
    let body = if value.starts_with(['{', '[']) {
        let tokens = source_tokens(value, imports.is_kotlin);
        let end = matching(&tokens, 0)?;
        if end + 1 != tokens.len() {
            return None;
        }
        &value[1..tokens[end].start]
    } else {
        value
    };
    let mut result = BTreeSet::new();
    if body.trim().is_empty() {
        return Some(result);
    }
    for (a, b) in segments(body) {
        let original = body[a..b].trim();
        let mut name = original;
        let canonical;
        if imports.is_kotlin {
            let root = name.split('.').next()?;
            if imports.ambiguous_explicit.contains_key(root) {
                return None;
            }
            canonical = if let Some(explicit) = imports.explicit.get(root) {
                format!("{explicit}{}", &name[root.len()..])
            } else if name.starts_with("org.spongepowered.asm.mixin.injection.Constant.Condition.")
            {
                name.into()
            } else if root == "Constant"
                && resolve_annotation(root, imports) == Some(MixinAnnotation::Constant)
            {
                format!(
                    "org.spongepowered.asm.mixin.injection.Constant{}",
                    &name[root.len()..]
                )
            } else if !name.contains('.')
                && imports
                    .wildcard_packages
                    .iter()
                    .any(|p| p == "org.spongepowered.asm.mixin.injection.Constant.Condition")
            {
                format!("org.spongepowered.asm.mixin.injection.Constant.Condition.{name}")
            } else {
                return None;
            };
            name = canonical.rsplit('.').next()?;
            if canonical.rsplit_once('.')?.0
                != "org.spongepowered.asm.mixin.injection.Constant.Condition"
            {
                return None;
            }
        }
        let pair = match name.rsplit('.').next()? {
            "LESS_THAN_ZERO" => [155, 156],
            "LESS_THAN_OR_EQUAL_TO_ZERO" => [158, 157],
            "GREATER_THAN_ZERO" => [157, 158],
            "GREATER_THAN_OR_EQUAL_TO_ZERO" => [156, 155],
            _ => return None,
        };
        result.extend(pair);
    }
    Some(result)
}
fn boxed_generic(descriptor: &str) -> Option<&'static str> {
    Some(match descriptor {
        "I" => "Integer",
        "Z" => "Boolean",
        "J" => "Long",
        "F" => "Float",
        "D" => "Double",
        "B" => "Byte",
        "C" => "Character",
        "S" => "Short",
        "V" => "Void",
        _ => return None,
    })
}
fn descriptor_from_handler_type(
    name: &str,
    index: &dyn ClassIndex,
    imports: Option<&JavaSourceImports>,
    reference_types: Option<&dyn HandlerReferenceTypeResolver>,
) -> Option<String> {
    if let Some(imports) = imports {
        let resolved = JavaTypeDescriptorResolver::descriptor_or_null(
            name,
            &JavaTypeResolutionContext {
                imports,
                lookup: Some(&ClassIndexJavaTypeLookup { class_index: index }),
            },
        );
        if resolved.is_some() || imports.is_kotlin {
            return resolved;
        }
    }
    if let Some(value) = OperationSignatureRenderer::descriptor_from_type_name(name, index) {
        return Some(value);
    }
    let imports = JavaSourceImports {
        explicit: index
            .find_classes("", 100_000)
            .into_iter()
            .map(|c| (c.simple_name.clone(), c.fqn()))
            .collect(),
        ..Default::default()
    };
    if let Some(value) = JavaTypeDescriptorResolver::descriptor_or_null(
        name,
        &JavaTypeResolutionContext {
            imports: &imports,
            lookup: None,
        },
    ) {
        return Some(value);
    }
    let mut normalized = name.trim();
    let mut arrays = 0;
    while let Some(component) = normalized.strip_suffix("[]") {
        arrays += 1;
        normalized = component.trim();
    }
    normalized = normalized.split('<').next().unwrap_or("").trim();
    let candidates = if normalized.contains('.') {
        vec![normalized.to_string()]
    } else {
        vec![format!("java.lang.{normalized}"), normalized.to_string()]
    };
    for candidate in candidates {
        let internal = candidate.replace('.', "/");
        if reference_types.and_then(|r| r.is_known_reference(&internal)) == Some(true) {
            return Some(format!("{}L{internal};", "[".repeat(arrays)));
        }
    }
    None
}

fn segments(value: &str) -> Vec<(usize, usize)> {
    let mut start = 0;
    let mut result = vec![];
    let (mut angle, mut paren, mut brace, mut bracket) = (0i32, 0i32, 0i32, 0i32);
    for t in source_tokens(value, true)
        .into_iter()
        .filter(|t| t.kind == SourceTokenKind::Symbol)
    {
        match t.text.as_str() {
            "<" => angle += 1,
            ">" => angle = (angle - 1).max(0),
            "(" => paren += 1,
            ")" => paren = (paren - 1).max(0),
            "{" => brace += 1,
            "}" => brace = (brace - 1).max(0),
            "[" => bracket += 1,
            "]" => bracket = (bracket - 1).max(0),
            "," if angle == 0 && paren == 0 && brace == 0 && bracket == 0 => {
                result.push((start, t.start));
                start = t.end;
            }
            _ => {}
        }
    }
    result.push((start, value.len()));
    result
}
fn parsed_string(value: &str, kotlin: bool) -> Option<String> {
    let v = value.trim();
    if kotlin && v.len() >= 6 && v.starts_with("\"\"\"") && v.ends_with("\"\"\"") {
        return Some(v[3..v.len() - 3].into());
    }
    if v.contains(['\n', '\r']) {
        return None;
    }
    let content = v.strip_prefix('"')?.strip_suffix('"')?;
    let tokens = source_tokens(v, false);
    if tokens.len() != 1 {
        return None;
    }
    {
        let mut result = String::new();
        let mut i = 0;
        while i < content.len() {
            if content.as_bytes()[i] == 92 && content.as_bytes().get(i + 1) == Some(&117) {
                let decoded =
                    JavaStringContentDecoder::decode_unicode_escape(content, i, content.len())?;
                result.push(decoded.r#char);
                i = decoded.next_index;
            } else {
                i = JavaStringContentDecoder::append_decoded_char(
                    content,
                    i,
                    content.len(),
                    &mut result,
                )?;
            }
        }
        Some(result)
    }
}
fn string_values(value: &str, kotlin: bool) -> Vec<String> {
    let value = value.trim();
    if value.starts_with(['{', '[']) {
        let close = if value.starts_with('[') { ']' } else { '}' };
        let Some(end) = value.rfind(close) else {
            return vec![];
        };
        segments(&value[1..end])
            .into_iter()
            .filter_map(|(a, b)| parsed_string(&value[1 + a..1 + b], kotlin))
            .collect()
    } else {
        parsed_string(value, kotlin).into_iter().collect()
    }
}
fn resolve_annotation(name: &str, imports: &JavaSourceImports) -> Option<MixinAnnotation> {
    if name.contains('.') {
        return MixinAnnotation::from_official_fqn(name);
    }
    if imports.ambiguous_explicit.contains_key(name) {
        return None;
    }
    if let Some(f) = imports.explicit.get(name) {
        return MixinAnnotation::from_official_fqn(f);
    }
    let annotation = MixinAnnotation::from_simple_name(name)?;
    (!imports.is_kotlin
        || annotation.official_fqns().iter().any(|f| {
            f.rsplit_once('.').is_some_and(|(p, _)| {
                imports.wildcard_packages.iter().any(|w| w == p)
                    || imports.package_name.as_deref() == Some(p)
            })
        }))
    .then_some(annotation)
}
fn official_name(name: &str, official: &str, imports: &JavaSourceImports) -> bool {
    if name.contains('.') {
        name == official
    } else if let Some(f) = imports.explicit.get(name) {
        f == official
    } else {
        name == official.rsplit('.').next().unwrap()
            && (!imports.is_kotlin
                || imports
                    .wildcard_packages
                    .iter()
                    .any(|p| official.rsplit_once('.').unwrap().0 == p))
    }
}
fn class_literal(value: &str) -> Option<String> {
    let value = value.trim();
    let name = value
        .strip_suffix(".class")
        .or_else(|| value.strip_suffix("::class"))?
        .trim();
    if name.is_empty()
        || !name.split('.').all(|p| {
            let mut chars = p.chars();
            chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        })
    {
        return None;
    }
    Some(name.into())
}
fn attributes(value: &str) -> Vec<(&str, &str)> {
    segments(value)
        .into_iter()
        .filter_map(|(start, end)| {
            let text = trim_comments(value[start..end].trim());
            let at = source_tokens(text, true)
                .into_iter()
                .find(|t| t.text == "=")?
                .start;
            Some((text[..at].trim(), text[at + 1..].trim()))
        })
        .collect()
}
#[derive(Default)]
struct StrippedParameter {
    text: String,
    sugar: Option<HandlerParameterSugarSpec>,
    sugar_range: Option<(usize, usize)>,
    has_coerce: bool,
    coerce_range: Option<(usize, usize)>,
}
fn strip_parameter_annotations(parameter: &str, imports: &JavaSourceImports) -> StrippedParameter {
    let tokens = source_tokens(parameter, imports.is_kotlin);
    let mut result = StrippedParameter::default();
    let mut retained = 0;
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].text != "@" {
            i += 1;
            continue;
        }
        let start = tokens[i].start;
        let mut next = i + 1;
        let mut name = String::new();
        while next < tokens.len()
            && (tokens[next].kind == SourceTokenKind::Identifier || tokens[next].text == ".")
        {
            name.push_str(&tokens[next].text);
            next += 1;
            if tokens.get(next).is_none_or(|t| t.text != ".")
                && tokens.get(next - 1).is_some_and(|t| t.text != ".")
            {
                break;
            }
        }
        if next == i + 1 {
            i += 1;
            continue;
        }
        let name_end = tokens[next - 1].end;
        let mut end = name_end;
        let body = if tokens.get(next).is_some_and(|t| t.text == "(") {
            if let Some(close) = matching(&tokens, next) {
                end = tokens[close].end;
                let body = &parameter[tokens[next].end..tokens[close].start];
                next = close + 1;
                Some(body)
            } else {
                end = parameter.len();
                next = tokens.len();
                None
            }
        } else {
            None
        };
        let coerce = official_name(&name, COERCE_FQN, imports);
        let sugar = match resolve_annotation(&name, imports) {
            Some(MixinAnnotation::Local) => Some(parse_local_sugar(body.unwrap_or(""))),
            Some(MixinAnnotation::Share) => Some(parse_share_sugar(body.unwrap_or(""))),
            _ if official_name(&name, CANCELLABLE_FQN, imports) => {
                Some(HandlerParameterSugarSpec::Cancellable)
            }
            _ => None,
        };
        if coerce || sugar.is_some() || imports.is_kotlin {
            result.text.push_str(&parameter[retained..start]);
            result.text.push(' ');
            retained = end;
            if coerce {
                result.has_coerce = true;
                if result.coerce_range.is_none() {
                    result.coerce_range = Some((start, end));
                }
            }
            if let Some(sugar) = sugar {
                result.sugar = Some(sugar);
                result.sugar_range = Some((start, end));
            }
        }
        i = next;
    }
    result.text.push_str(&parameter[retained..]);
    result.text = result.text.trim().into();
    result
}
fn parse_local_sugar(body: &str) -> HandlerParameterSugarSpec {
    let mut spec = LocalSugarSpec::default();
    for (key, value) in attributes(body) {
        match key {
            "argsOnly" => spec.args_only = value.eq_ignore_ascii_case("true"),
            "index" => spec.index = value.parse::<i32>().ok().filter(|n| *n != -1),
            "ordinal" => spec.ordinal = value.parse::<i32>().ok().filter(|n| *n != -1),
            "name" => spec.names.extend(string_values(value, false)),
            "print" => spec.print = value.eq_ignore_ascii_case("true"),
            "type" => {
                if let Some(name) = class_literal(value) {
                    spec.type_class_name = Some(name);
                }
            }
            _ => {}
        }
    }
    HandlerParameterSugarSpec::Local(spec)
}
fn parse_share_sugar(body: &str) -> HandlerParameterSugarSpec {
    let mut spec = ShareSugarSpec::default();
    if let Some(value) = parsed_string(body, false) {
        spec.value = Some(value);
    } else {
        for (key, value) in attributes(body) {
            match key {
                "value" => spec.value = parsed_string(value, false),
                "namespace" => {
                    spec.namespace = parsed_string(value, false);
                    spec.namespace_specified = true;
                }
                _ => {}
            }
        }
    }
    HandlerParameterSugarSpec::Share(spec)
}
fn parse_parameters(
    source: &str,
    start: usize,
    end: usize,
    imports: &JavaSourceImports,
) -> Vec<HandlerParameterDeclaration> {
    let raw = &source[start..end];
    segments(raw)
        .into_iter()
        .filter_map(|(a, b)| {
            let raw_segment = &raw[a..b];
            let trimmed = raw_segment.trim();
            if trimmed.is_empty() {
                return None;
            }
            let first = start + a + raw_segment.len() - raw_segment.trim_start().len();
            let last = first + trimmed.len();
            let stripped = strip_parameter_annotations(trimmed, imports);
            let parseable = AnnotationContextExtractor::mask_non_code(&stripped.text);
            let parseable = parseable.trim();
            let (generic, mut type_name, name) = if imports.is_kotlin {
                let (name, ty) = parseable.split_once(':')?;
                let name = name
                    .trim()
                    .strip_prefix("vararg ")
                    .unwrap_or(name.trim())
                    .trim()
                    .trim_matches('`')
                    .to_string();
                let vararg = if parseable.trim_start().starts_with("vararg ") {
                    "vararg "
                } else {
                    ""
                };
                let ty = ty.split('=').next().unwrap_or("").trim();
                (None, format!("{vararg}{ty}"), name)
            } else {
                let generic = find_java_operation_generic(parseable);
                let (ty, name) = parseable.rsplit_once(' ').unwrap_or((parseable, parseable));
                let type_name = generic
                    .as_ref()
                    .map_or_else(|| ty.trim().into(), |g| format!("Operation<{g}>"));
                (generic, type_name, name.trim().into())
            };
            let erased = type_name
                .split('<')
                .next()
                .unwrap_or("")
                .trim_end_matches('?')
                .trim();
            let is_operation = if imports.is_kotlin {
                imports
                    .explicit
                    .get(erased)
                    .map_or(erased == OPERATION_FQN, |f| f == OPERATION_FQN)
                    || (erased == "Operation"
                        && !imports.explicit.contains_key(erased)
                        && imports
                            .wildcard_packages
                            .iter()
                            .any(|p| p == OPERATION_FQN.rsplit_once('.').unwrap().0))
            } else {
                generic.is_some()
                    || parseable.contains("Operation<")
                    || erased == "Operation"
                    || erased == OPERATION_FQN
            };
            let operation_generic_name =
                if imports.is_kotlin && is_operation && type_name.contains('<') {
                    type_name
                        .split_once('<')
                        .and_then(|(_, v)| v.rsplit_once('>').map(|(v, _)| v.trim().into()))
                } else {
                    generic
                };
            Some(HandlerParameterDeclaration {
                name,
                type_name: std::mem::take(&mut type_name),
                type_descriptor: None,
                is_operation,
                operation_generic_name,
                is_sugar: stripped.sugar.is_some(),
                sugar_spec: stripped.sugar,
                range: Some(source_range(source, first, last)),
                sugar_annotation_range: stripped
                    .sugar_range
                    .map(|(a, b)| source_range(source, first + a, first + b)),
                has_coerce: stripped.has_coerce,
                coerce_annotation_range: stripped
                    .coerce_range
                    .map(|(a, b)| source_range(source, first + a, first + b)),
                operation_generic_descriptor: None,
            })
        })
        .collect()
}
fn find_java_operation_generic(source: &str) -> Option<String> {
    let offset = source.find("Operation")? + "Operation".len();
    let rest = source[offset..].trim_start();
    let inner = rest.strip_prefix('<')?;
    let mut depth = 1;
    for (i, c) in inner.char_indices() {
        if c == '<' {
            depth += 1;
        } else if c == '>' {
            depth -= 1;
            if depth == 0 {
                let name = inner[..i].trim();
                return (!name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_alphanumeric() || "_<>[].".contains(c)))
                .then(|| name.into());
            }
        }
    }
    None
}
pub mod diagnostic_codes {
    pub const INVALID_DEFINITION: &str = "MIXINEXTRAS_INVALID_DEFINITION";
    pub const HANDLER_SIGNATURE_MISMATCH: &str = "MIXINEXTRAS_HANDLER_SIGNATURE_MISMATCH";
    pub const MISSING_OPERATION_PARAMETER: &str = "MIXINEXTRAS_MISSING_OPERATION_PARAMETER";
    pub const WRONG_OPERATION_GENERIC: &str = "MIXINEXTRAS_WRONG_OPERATION_GENERIC";
    pub const WRONG_ORIGINAL_VALUE_TYPE: &str = "MIXINEXTRAS_WRONG_ORIGINAL_VALUE_TYPE";
    pub const WRONG_RETURN_TYPE: &str = "MIXINEXTRAS_WRONG_RETURN_TYPE";
    pub const UNSUPPORTED_EXPRESSION_CONTEXT: &str = "MIXINEXTRAS_UNSUPPORTED_EXPRESSION_CONTEXT";
    pub const MISSING_EXPRESSION_ANNOTATION: &str = "MIXINEXTRAS_MISSING_EXPRESSION_ANNOTATION";
    pub const INVALID_SHARE_PARAMETER_TYPE: &str = "MIXINEXTRAS_INVALID_SHARE_PARAMETER_TYPE";
    pub const UNSUPPORTED_SUGAR_PARAMETER: &str = "MIXINEXTRAS_UNSUPPORTED_SUGAR_PARAMETER";
    pub const WRAP_WITH_CONDITION_INVALID_TARGET: &str =
        "MIXINEXTRAS_WRAP_WITH_CONDITION_INVALID_TARGET";
    pub const WRAP_WITH_CONDITION_POPPED_NON_VOID: &str =
        "MIXINEXTRAS_WRAP_WITH_CONDITION_POPPED_NON_VOID";
    pub const WRONG_OPERATION_CALL_ARGUMENTS: &str = "MIXINEXTRAS_WRONG_OPERATION_CALL_ARGUMENTS";
    pub const LOCAL_CAPTURE_NOT_FOUND: &str = "MIXINEXTRAS_LOCAL_CAPTURE_NOT_FOUND";
    pub const LOCAL_CAPTURE_AMBIGUOUS: &str = "MIXINEXTRAS_LOCAL_CAPTURE_AMBIGUOUS";
    pub const SHARE_TYPE_CONFLICT: &str = "MIXINEXTRAS_SHARE_TYPE_CONFLICT";
    pub const WRAP_METHOD_STATIC_MISMATCH: &str = "MIXINEXTRAS_WRAP_METHOD_STATIC_MISMATCH";
    pub const WRAP_METHOD_INVALID_TARGET: &str = "MIXINEXTRAS_WRAP_METHOD_INVALID_TARGET";
}
