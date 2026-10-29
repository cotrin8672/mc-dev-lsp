//! Exact positive definition locations from the evaluation's own document.
use super::index::EvaluationFactReader;
use crate::{
    host_facts::{draft::TypeJvm, inputs::read_split, FactsError},
    wire::{
        AnnotationValue, Declaration, Evaluation, Fact, HandleKind, Member, Registry, TypeFact,
    },
};
use mcdev_core::{
    mixin_navigation::{
        DefinitionBackend, DefinitionResolution, DefinitionResolutionService,
        MixinDefinitionService, ResolvedDefinition,
    },
    mixin_source::{AnnotationContextExtractor, AnnotationSlot, MixinAnnotation},
    model::{
        offset_to_position, position_to_offset, McDefinitionTarget, McTextPosition, McTextRange,
        MemberKind,
    },
    project::ProjectContext,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Mutex};

/// Resolve a positive local Expression reference only when the host confirms both
/// annotations, their arguments, and their common declaration in this evaluation.
pub(super) fn expression_definition<R: EvaluationFactReader + Clone>(
    evaluation: &Evaluation,
    uri: &str,
    line: usize,
    character: usize,
    reader: R,
) -> Result<Option<mcdev_protocol::McdevDefinitionResponse>, FactsError> {
    let source = &evaluation.source;
    let Some(offset) = position_to_offset(source, McTextPosition { line, character }) else {
        return Ok(None);
    };
    let Some(context) = AnnotationContextExtractor::extract_at_offset(source, offset) else {
        return Ok(None);
    };
    if context.annotation != MixinAnnotation::Expression || context.slot != AnnotationSlot::Value {
        return Ok(None);
    }
    let candidates: Vec<_> = evaluation
        .declarations
        .iter()
        .filter_map(|d| {
            let expressions: Vec<_> = d
                .annotations
                .iter()
                .filter(|a| {
                    a.range.start == context.annotation_start_offset
                        && a.range.end == context.annotation_end_offset
                })
                .collect();
            (expressions.len() == 1).then(|| (d, expressions[0]))
        })
        .collect();
    if candidates.len() != 1 {
        return Ok(None);
    }
    let (declaration, expression) = candidates[0];
    let expr_args: Vec<_> = expression
        .arguments
        .iter()
        .filter(|a| a.name.as_deref().is_none_or(|n| n == "value"))
        .collect();
    if expr_args.len() != 1 {
        return Ok(None);
    }
    let expr_arg = expr_args[0];
    let Some(AnnotationValue::String {
        value: expr,
        content_range,
    }) = expr_arg.value.found()
    else {
        return Ok(None);
    };
    if content_range.start < expression.range.start
        || content_range.end > expression.range.end
        || content_range.start < expr_arg.range.start
        || content_range.end > expr_arg.range.end
        || offset < content_range.start
        || offset >= content_range.end
        || content_range.text(source).ok() != Some(expr.as_str())
    {
        return Ok(None);
    }
    let queries: Vec<_> = declaration
        .annotations
        .iter()
        .map(|a| json!({"op":"resolveTypeRef","reference":a.type_ref}))
        .collect();
    reader.check_active()?;
    let answers = read_split(&mut reader.clone(), &evaluation.evaluation, &queries)?;
    reader.check_active()?;
    let mut registry = Registry::default();
    evaluation
        .validate(&mut registry)
        .map_err(FactsError::Protocol)?;
    let mut types = Vec::new();
    for answer in answers {
        let fact: Fact<TypeFact> = serde_json::from_value(answer)
            .map_err(|e| FactsError::Protocol(format!("invalid annotation type fact: {e}")))?;
        fact.validate().map_err(FactsError::Protocol)?;
        if let Some(ty) = fact.found() {
            if ty.qualified_name.is_empty() {
                return Err(FactsError::Protocol("empty host type name".into()));
            }
            registry
                .register(&ty.id, HandleKind::Type)
                .map_err(FactsError::Protocol)?;
        }
        types.push(fact);
    }
    let expr_index = declaration
        .annotations
        .iter()
        .position(|a| std::ptr::eq(a, expression))
        .unwrap();
    if types[expr_index]
        .found()
        .is_none_or(|ty| ty.qualified_name != "com.llamalad7.mixinextras.expression.Expression")
    {
        return Ok(None);
    }
    // Unknown annotation identities cannot prove that the member's ID is unique.
    if types.iter().any(|t| t.found().is_none()) {
        return Ok(None);
    }
    let targets = std::panic::catch_unwind(|| {
        MixinDefinitionService::new(&NO_INDEX, &NO_INDEX).definitions_at(
            source,
            line,
            character,
            None,
            Some(uri),
        )
    });
    let targets = match targets {
        Ok(targets) => targets,
        Err(payload) if payload.is::<IndexNeeded>() => return Ok(None),
        Err(payload) => std::panic::resume_unwind(payload),
    };
    if targets.len() != 1 {
        return Ok(None);
    }
    let target = &targets[0];
    let Some(name) = target.name.as_deref() else {
        return Ok(None);
    };
    let Some(target_range) = target.source_range else {
        return Ok(None);
    };
    for (annotation, ty) in declaration.annotations.iter().zip(&types) {
        if ty
            .found()
            .is_some_and(|t| t.qualified_name == "com.llamalad7.mixinextras.expression.Definition")
        {
            let ids: Vec<_> = annotation
                .arguments
                .iter()
                .filter(|a| a.name.as_deref() == Some("id"))
                .collect();
            if ids.len() != 1
                || !matches!(ids[0].value.found(), Some(AnnotationValue::String { .. }))
            {
                return Ok(None);
            }
        }
    }
    let matching: Vec<_> = declaration.annotations.iter().zip(&types).filter(|(a, t)| {
        t.found().is_some_and(|ty| ty.qualified_name == "com.llamalad7.mixinextras.expression.Definition") && a.arguments.iter().any(|arg| {
            arg.name.as_deref() == Some("id") && matches!(arg.value.found(), Some(AnnotationValue::String { value, content_range }) if value == name && content_range.start >= a.range.start && content_range.end <= a.range.end && content_range.start >= arg.range.start && content_range.end <= arg.range.end && content_range.text(source).ok() == Some(value.as_str()) && offset_to_position(source, content_range.start) == Some(target_range.start) && offset_to_position(source, content_range.end) == Some(target_range.end))
        })
    }).collect();
    if matching.len() != 1 || target.direct_source_document_uri.as_deref() != Some(uri) {
        return Ok(None);
    }
    let context =
        mcdev_core::project::ProjectContextBuilder::empty("evaluation", std::path::PathBuf::new());
    let resolved = DefinitionResolutionService::new(vec![]).resolve_all(&targets, &context, uri);
    let resolved: Vec<_> = resolved
        .into_iter()
        .map(super::super::core::convert_definition)
        .collect();
    let locations = mcdev_protocol::convert::DefinitionConverter::to_locations(&resolved)
        .map_err(|e| FactsError::Protocol(format!("invalid definition response: {e}")))?;
    reader.check_active()?;
    Ok(Some(mcdev_protocol::McdevDefinitionResponse { locations }))
}

struct IndexNeeded;
struct NoIndex;
static NO_INDEX: NoIndex = NoIndex;
impl mcdev_core::model::ClassIndex for NoIndex {
    fn find_classes(&self, _: &str, _: usize) -> Vec<mcdev_core::model::ClassIndexEntry> {
        std::panic::panic_any(IndexNeeded)
    }
    fn find_class(&self, _: &str) -> Option<mcdev_core::model::ClassIndexEntry> {
        std::panic::panic_any(IndexNeeded)
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<mcdev_core::model::ClassIndexEntry> {
        std::panic::panic_any(IndexNeeded)
    }
    fn get_methods(&self, _: &str) -> Vec<mcdev_core::model::MethodIndexEntry> {
        std::panic::panic_any(IndexNeeded)
    }
    fn get_fields(&self, _: &str) -> Vec<mcdev_core::model::FieldIndexEntry> {
        std::panic::panic_any(IndexNeeded)
    }
}
impl mcdev_core::bytecode::BytecodeIndex for NoIndex {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<mcdev_core::bytecode::AtTargetCandidate> {
        std::panic::panic_any(IndexNeeded)
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        std::panic::panic_any(IndexNeeded)
    }
}

#[derive(Clone)]
struct Owner {
    declaration: Declaration,
    qualified_name: String,
}

#[derive(Clone)]
struct MemberInventory {
    members: Vec<(Declaration, Member)>,
    incomplete: bool,
}

/// A positive match uses only declarations and type identities issued for this evaluation.
pub struct CurrentDefinitions<R: EvaluationFactReader + Clone> {
    evaluation: Evaluation,
    document_uri: String,
    reader: R,
    owners: Mutex<BTreeMap<String, Owner>>,
    members: Mutex<BTreeMap<String, MemberInventory>>,
}

impl<R: EvaluationFactReader + Clone> CurrentDefinitions<R> {
    pub fn new(
        evaluation: Evaluation,
        document_uri: String,
        reader: R,
    ) -> Result<Self, FactsError> {
        evaluation
            .validate(&mut Registry::default())
            .map_err(FactsError::Protocol)?;
        reader.check_active()?;
        Ok(Self {
            evaluation,
            document_uri,
            reader,
            owners: Mutex::new(BTreeMap::new()),
            members: Mutex::new(BTreeMap::new()),
        })
    }

    fn read<T: DeserializeOwned>(&self, queries: &[Value]) -> Result<Vec<T>, FactsError> {
        self.reader.check_active()?;
        let values = read_split(
            &mut self.reader.clone(),
            &self.evaluation.evaluation,
            queries,
        )?;
        self.reader.check_active()?;
        values
            .into_iter()
            .map(|value| {
                serde_json::from_value(value).map_err(|error| {
                    FactsError::Protocol(format!("invalid definition fact: {error}"))
                })
            })
            .collect()
    }

    fn owner(&self, internal_name: &str) -> Result<Owner, FactsError> {
        self.reader.check_active()?;
        if let Some(owner) = self
            .owners
            .lock()
            .map_err(|_| protocol("definition cache poisoned"))?
            .get(internal_name)
            .cloned()
        {
            return Ok(owner);
        }
        // This is only a candidate filter; each positive match is checked against host typeJvm below.
        let suffix = internal_name.rsplit('/').next().unwrap_or("");
        let nested_suffix = suffix.rsplit('$').next().unwrap_or(suffix);
        let candidates: Vec<_> = self
            .evaluation
            .declarations
            .iter()
            .filter(|d| {
                d.name.as_deref().is_some_and(|name| {
                    let name = name.trim_matches('`');
                    name == suffix || name == nested_suffix
                })
            })
            .cloned()
            .collect();
        if candidates.is_empty() {
            return Err(unknown("target is not in the current document"));
        }
        let queries: Vec<_> = candidates
            .iter()
            .map(|d| json!({"op":"declarationType","declaration":d.id}))
            .collect();
        let type_facts: Vec<Fact<TypeFact>> = self.read(&queries)?;
        let mut possible = Vec::new();
        for (declaration, fact) in candidates.into_iter().zip(type_facts) {
            fact.validate().map_err(FactsError::Protocol)?;
            if let Fact::Found { value } = fact {
                possible.push((declaration, value));
            }
        }
        let queries: Vec<_> = possible
            .iter()
            .map(|(_, ty)| json!({"op":"typeJvm","target":ty.id}))
            .collect();
        let jvm: Vec<TypeJvm> = self.read(&queries)?;
        let mut matching = Vec::new();
        for ((declaration, ty), jvm) in possible.into_iter().zip(jvm) {
            jvm.binary_name.validate().map_err(FactsError::Protocol)?;
            if let Fact::Found { value: name } = jvm.binary_name {
                if name.internal_name == internal_name
                    && name.descriptor == format!("L{internal_name};")
                {
                    matching.push(Owner {
                        declaration,
                        qualified_name: ty.qualified_name,
                    });
                }
            }
        }
        let [owner] = matching.as_slice() else {
            return Err(unknown("current document has no unique matching host type"));
        };
        // Host resolution checks ambiguity in the selected scope; equal names alone are not authority.
        let resolved: Vec<Fact<TypeFact>> =
            self.read(&[json!({"op":"resolveType","name":owner.qualified_name})])?;
        let resolved = resolved
            .into_iter()
            .next()
            .ok_or_else(|| protocol("missing resolved type fact"))?;
        resolved.validate().map_err(FactsError::Protocol)?;
        let Fact::Found { value: resolved } = resolved else {
            return Err(unknown("host type resolution is incomplete"));
        };
        if resolved.qualified_name != owner.qualified_name {
            return Err(protocol("resolved host type name mismatch"));
        }
        let resolved_jvm: Vec<TypeJvm> =
            self.read(&[json!({"op":"typeJvm","target":resolved.id})])?;
        let selected = resolved_jvm
            .into_iter()
            .next()
            .ok_or_else(|| protocol("missing resolved JVM fact"))?;
        selected
            .binary_name
            .validate()
            .map_err(FactsError::Protocol)?;
        if !selected.binary_name.found().is_some_and(|name| {
            name.internal_name == internal_name && name.descriptor == format!("L{internal_name};")
        }) {
            return Err(unknown(
                "selected host type does not match the target JVM identity",
            ));
        }
        self.reader.check_active()?;
        self.owners
            .lock()
            .map_err(|_| protocol("definition cache poisoned"))?
            .insert(internal_name.to_owned(), owner.clone());
        Ok(owner.clone())
    }

    fn members(&self, owner: &Owner) -> Result<MemberInventory, FactsError> {
        self.reader.check_active()?;
        if let Some(members) = self
            .members
            .lock()
            .map_err(|_| protocol("definition cache poisoned"))?
            .get(&owner.declaration.id)
            .cloned()
        {
            return Ok(members);
        }
        let candidates: Vec<_> = self
            .evaluation
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.owner.as_deref() == Some(owner.declaration.id.as_str())
            })
            .cloned()
            .collect();
        let queries: Vec<_> = candidates
            .iter()
            .map(|d| json!({"op":"declarationMember","declaration":d.id}))
            .collect();
        let facts: Vec<Fact<Member>> = self.read(&queries)?;
        let mut members = Vec::new();
        let mut incomplete = false;
        for (declaration, fact) in candidates.into_iter().zip(facts) {
            fact.validate().map_err(FactsError::Protocol)?;
            match fact {
                Fact::Found { value } => {
                    value.signature.validate().map_err(FactsError::Protocol)?;
                    value.dispatch.validate().map_err(FactsError::Protocol)?;
                    if value.id.is_empty()
                        || value.name.is_empty()
                        || !matches!(value.kind.as_str(), "method" | "field")
                        || value
                            .signature
                            .found()
                            .is_some_and(|signature| signature.kind != value.kind)
                    {
                        return Err(protocol("invalid host member identity"));
                    }
                    members.push((declaration, value));
                }
                Fact::Unknown { .. } | Fact::DefinitelyAbsent => incomplete = true,
            }
        }
        let members = MemberInventory {
            members,
            incomplete,
        };
        self.reader.check_active()?;
        self.members
            .lock()
            .map_err(|_| protocol("definition cache poisoned"))?
            .insert(owner.declaration.id.clone(), members.clone());
        Ok(members)
    }

    fn checked_resolve(
        &self,
        target: &McDefinitionTarget,
    ) -> Result<ResolvedDefinition, FactsError> {
        let owner = self.owner(&target.owner_internal_name)?;
        let declaration = match target.kind {
            MemberKind::Class => owner.declaration,
            MemberKind::Method | MemberKind::Field => {
                let name = target
                    .name
                    .as_deref()
                    .ok_or_else(|| unknown("target member name is unavailable"))?;
                let expected_kind = if target.kind == MemberKind::Method {
                    "method"
                } else {
                    "field"
                };
                let members = self.members(&owner)?;
                if members.incomplete {
                    return Err(unknown("host member inventory is incomplete"));
                }
                let matches: Vec<_> = members
                    .members
                    .into_iter()
                    .filter(|(_, member)| {
                        if member.kind != expected_kind || member.name != name {
                            return false;
                        }
                        let Some(expected) = target
                            .descriptor
                            .as_deref()
                            .filter(|value| !value.is_empty())
                        else {
                            return true;
                        };
                        let Some(actual) = member
                            .signature
                            .found()
                            .map(|signature| signature.descriptor.as_str())
                        else {
                            return false;
                        };
                        if target.kind == MemberKind::Method {
                            actual.split(')').next() == expected.split(')').next()
                        } else {
                            actual == expected
                        }
                    })
                    .collect();
                let [(declaration, member)] = matches.as_slice() else {
                    return Err(unknown(
                        "current document has no unique matching host member",
                    ));
                };
                member.signature.validate().map_err(FactsError::Protocol)?;
                if member.signature.found().is_none() {
                    return Err(unknown("host member signature is unavailable"));
                }
                declaration.clone()
            }
            _ => return Err(unknown("unsupported definition target kind")),
        };
        let start = offset_to_position(&self.evaluation.source, declaration.range.start)
            .ok_or_else(|| protocol("invalid source declaration range"))?;
        let end = offset_to_position(&self.evaluation.source, declaration.range.end)
            .ok_or_else(|| protocol("invalid source declaration range"))?;
        self.reader.check_active()?;
        Ok(ResolvedDefinition {
            target: target.clone(),
            document_uri: self.document_uri.clone(),
            range: McTextRange { start, end },
            resolution: DefinitionResolution::Source,
            resolution_message: None,
        })
    }
}

fn protocol(value: &str) -> FactsError {
    FactsError::Protocol(value.into())
}
fn unknown(value: &str) -> FactsError {
    FactsError::Unknown(value.into())
}

impl<R: EvaluationFactReader + Clone> DefinitionBackend for CurrentDefinitions<R> {
    fn resolve(
        &self,
        target: &McDefinitionTarget,
        _context: &ProjectContext,
        _workspace_root: &str,
    ) -> Option<ResolvedDefinition> {
        match self.checked_resolve(target) {
            Ok(result) => Some(result),
            Err(error) => std::panic::panic_any(error.command()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_facts::bridge::FactReader;
    use mcdev_core::{model::McTextPosition, MappingNamespace};
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    #[derive(Clone)]
    struct Reader(Arc<Mutex<VecDeque<Value>>>);
    impl FactReader for Reader {
        fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
            assert_eq!(evaluation, "eval-1");
            assert!(!queries.is_empty());
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| protocol("unexpected host read"))
        }
    }
    impl EvaluationFactReader for Reader {
        fn check_active(&self) -> Result<(), FactsError> {
            Ok(())
        }
    }
    fn found(value: Value) -> Value {
        json!({"state":"found","value":value})
    }
    fn unknown_fact() -> Value {
        json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
    }
    fn type_fact(id: &str) -> Value {
        found(json!({"id":id,"qualifiedName":"demo.Owner","package":"demo"}))
    }
    fn type_jvm() -> Value {
        json!({"binaryName":found(json!({"internalName":"demo/Owner","descriptor":"Ldemo/Owner;"})),
            "classFlags":unknown_fact(),"declaredMembers":{"members":[],"coverage":unknown_fact()},
            "abi":unknown_fact(),"jvmAbiCoverage":unknown_fact()})
    }
    fn member() -> Value {
        found(json!({"id":"member:1","name":"tick","kind":"method",
            "signature":found(json!({"id":"signature:1","kind":"method","descriptor":"()V"})),
            "dispatch":found(json!("instance"))}))
    }
    fn declaration(id: &str, name: &str, owner: Option<&str>, start: usize) -> Value {
        json!({"id":id,"name":name,"owner":owner,"range":{"start":start,"end":start+name.len()},"annotations":[]})
    }
    fn target() -> McDefinitionTarget {
        McDefinitionTarget {
            kind: MemberKind::Method,
            owner_internal_name: "demo/Owner".into(),
            owner_fqn: Some("demo.Owner".into()),
            name: Some("tick".into()),
            descriptor: Some("()V".into()),
            namespace: MappingNamespace::Named,
            source_range: None,
            direct_source_document_uri: None,
        }
    }
    fn evaluation(source: &str, extra_member: bool) -> Evaluation {
        let owner = source.find("Owner").unwrap();
        let first = source.find("tick").unwrap();
        let mut declarations = vec![
            declaration("class:1", "Owner", None, owner),
            declaration("decl:1", "tick", Some("class:1"), first),
        ];
        if extra_member {
            declarations.push(declaration(
                "decl:2",
                "tick",
                Some("class:1"),
                source.rfind("tick").unwrap(),
            ));
        }
        serde_json::from_value(json!({"evaluation":"eval-1","owner":"/Owner.java",
            "source":source,"configuration":{},"declarations":declarations}))
        .unwrap()
    }
    fn replies(member_facts: Vec<Value>) -> Reader {
        Reader(Arc::new(Mutex::new(VecDeque::from(vec![
            json!({"facts":[type_fact("type:1")]}),
            json!({"facts":[type_jvm()]}),
            json!({"facts":[type_fact("type:2")]}),
            json!({"facts":[type_jvm()]}),
            json!({"facts":member_facts}),
        ]))))
    }

    #[test]
    fn exact_host_member_returns_current_document_utf16_range_and_reuses_evaluation_facts() {
        let source = "class Owner { /* 😀 */ void tick() {} }";
        let reader = replies(vec![member()]);
        let backend = CurrentDefinitions::new(
            evaluation(source, false),
            "file:///Owner.java".into(),
            reader.clone(),
        )
        .unwrap();
        for _ in 0..2 {
            let result = backend.checked_resolve(&target()).unwrap();
            assert_eq!(result.document_uri, "file:///Owner.java");
            assert_eq!(
                result.range.start,
                McTextPosition {
                    line: 0,
                    character: 28
                }
            );
            assert_eq!(
                result.range.end,
                McTextPosition {
                    line: 0,
                    character: 32
                }
            );
            assert_eq!(result.resolution, DefinitionResolution::Source);
        }
        let context =
            mcdev_core::project::ProjectContextBuilder::empty("demo", std::env::temp_dir());
        let results =
            mcdev_core::mixin_navigation::DefinitionResolutionService::new(vec![&backend])
                .resolve_all(&[target()], &context, "file:///Owner.java");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_uri, "file:///Owner.java");
        assert_eq!(
            results[0].range.start,
            McTextPosition {
                line: 0,
                character: 28
            }
        );
        assert_eq!(results[0].resolution, DefinitionResolution::Source);
        assert!(reader.0.lock().unwrap().is_empty());
    }

    #[test]
    fn unknown_overload_is_not_discarded_to_choose_a_known_member() {
        let source = "class Owner { void tick() {} void tick(int n) {} }";
        let reader = replies(vec![member(), unknown_fact()]);
        let backend = CurrentDefinitions::new(
            evaluation(source, true),
            "file:///Owner.java".into(),
            reader,
        )
        .unwrap();
        assert!(matches!(
            backend.checked_resolve(&target()),
            Err(FactsError::Unknown(_))
        ));
    }

    fn expression_fixture() -> (Evaluation, Reader, usize, usize) {
        let source = "import com.llamalad7.mixinextras.expression.Definition;\nimport com.llamalad7.mixinextras.expression.Expression;\nimport com.llamalad7.mixinextras.injector.ModifyExpressionValue;\nclass Owner {\n @Definition(id = \"call\")\n @ModifyExpressionValue(method = \"draw()V\", at = @At(\"MIXINEXTRAS:EXPRESSION\"))\n @Expression(\"call\")\n void tick() {}\n}";
        let def_start = source.find("@Definition").unwrap();
        let exp_start = source.find("@Expression").unwrap();
        let def_value = source[def_start..].find("call").unwrap() + def_start;
        let exp_value = source[exp_start..].find("call").unwrap() + exp_start;
        let ann = |start: usize, end: usize, id: &str, arg: Option<&str>, value: usize| {
            json!({
                "typeRef":id,"range":{"start":start,"end":end},"arguments":[{"name":arg,
                "range":{"start":value-1,"end":value+5},"value":found(json!({"kind":"string","value":"call","contentRange":{"start":value,"end":value+4}}))}]
            })
        };
        let mut eval = evaluation(source, false);
        eval.declarations[1].annotations = serde_json::from_value(json!([
            ann(
                def_start,
                source[def_start..].find(')').unwrap() + def_start + 1,
                "ref:def",
                Some("id"),
                def_value
            ),
            ann(
                exp_start,
                source[exp_start..].find(')').unwrap() + exp_start + 1,
                "ref:expr",
                None,
                exp_value
            )
        ]))
        .unwrap();
        let answers = json!({"facts":[
            found(json!({"id":"type:def","qualifiedName":"com.llamalad7.mixinextras.expression.Definition"})),
            found(json!({"id":"type:expr","qualifiedName":"com.llamalad7.mixinextras.expression.Expression"}))
        ]});
        (
            eval,
            Reader(Arc::new(Mutex::new(VecDeque::from(vec![answers])))),
            def_value,
            exp_value,
        )
    }

    #[test]
    fn local_expression_uses_real_resolver_and_definition_converter() {
        let (eval, reader, def, expr) = expression_fixture();
        let pos = offset_to_position(&eval.source, expr + 2).unwrap();
        let result =
            expression_definition(&eval, "file:///Owner.java", pos.line, pos.character, reader)
                .unwrap()
                .unwrap();
        assert_eq!(result.locations.len(), 1);
        let location = &result.locations[0];
        assert_eq!(location.document_uri, "file:///Owner.java");
        assert_eq!(
            location.resolution,
            mcdev_protocol::McdevDefinitionResolution::Source
        );
        let expected = offset_to_position(&eval.source, def).unwrap();
        assert_eq!(location.range.start.line as usize, expected.line);
        assert_eq!(location.range.start.character as usize, expected.character);
        assert_eq!(
            location.metadata.get("name").map(String::as_str),
            Some("call")
        );
    }

    #[test]
    fn foreign_or_unknown_definition_is_not_promoted() {
        for identity in [
            found(json!({"id":"type:def","qualifiedName":"foreign.Definition"})),
            unknown_fact(),
        ] {
            let (eval, _, _, expr) = expression_fixture();
            let reader = Reader(Arc::new(Mutex::new(VecDeque::from(vec![
                json!({"facts":[identity,found(json!({"id":"type:expr","qualifiedName":"com.llamalad7.mixinextras.expression.Expression"}))]}),
            ]))));
            let pos = offset_to_position(&eval.source, expr + 2).unwrap();
            assert!(expression_definition(
                &eval,
                "file:///Owner.java",
                pos.line,
                pos.character,
                reader
            )
            .unwrap()
            .is_none());
        }
    }

    #[test]
    fn source_range_or_declaration_mismatch_does_not_resolve() {
        let (mut eval, reader, _, expr) = expression_fixture();
        if let Some(AnnotationValue::String { content_range, .. }) =
            match &mut eval.declarations[1].annotations[0].arguments[0].value {
                Fact::Found { value } => Some(value),
                _ => None,
            }
        {
            content_range.start += 1;
        }
        let pos = offset_to_position(&eval.source, expr + 2).unwrap();
        assert!(expression_definition(
            &eval,
            "file:///Owner.java",
            pos.line,
            pos.character,
            reader
        )
        .unwrap()
        .is_none());
        let (mut eval, _reader, _, expr) = expression_fixture();
        let definition = eval.declarations[1].annotations.remove(0);
        eval.declarations[0].annotations.push(definition);
        let pos = offset_to_position(&eval.source, expr + 2).unwrap();
        let reader = Reader(Arc::new(Mutex::new(VecDeque::from(vec![
            json!({"facts":[found(json!({"id":"type:expr","qualifiedName":"com.llamalad7.mixinextras.expression.Expression"}))]}),
        ]))));
        assert!(expression_definition(
            &eval,
            "file:///Owner.java",
            pos.line,
            pos.character,
            reader
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn expression_end_position_is_not_a_positive_reference() {
        let (eval, reader, _, expr) = expression_fixture();
        let pos = offset_to_position(&eval.source, expr + 4).unwrap();
        assert!(expression_definition(
            &eval,
            "file:///Owner.java",
            pos.line,
            pos.character,
            reader
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn ambiguous_host_definition_is_not_promoted() {
        let (mut eval, _, _, expr) = expression_fixture();
        let duplicate = eval.declarations[1].annotations[0].clone();
        eval.declarations[1].annotations.push(duplicate);
        let facts = json!({"facts":[
            found(json!({"id":"type:def","qualifiedName":"com.llamalad7.mixinextras.expression.Definition"})),
            found(json!({"id":"type:expr","qualifiedName":"com.llamalad7.mixinextras.expression.Expression"})),
            found(json!({"id":"type:def","qualifiedName":"com.llamalad7.mixinextras.expression.Definition"}))
        ]});
        let reader = Reader(Arc::new(Mutex::new(VecDeque::from(vec![facts]))));
        let pos = offset_to_position(&eval.source, expr + 2).unwrap();
        assert!(expression_definition(
            &eval,
            "file:///Owner.java",
            pos.line,
            pos.character,
            reader
        )
        .unwrap()
        .is_none());
    }
}
