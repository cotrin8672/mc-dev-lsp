use crate::{server::Failure, wire::*};
use mcdev_core::{
    descriptor::{parse_field_descriptor, parse_method_descriptor},
    mixin_source::MixinAnnotation,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(crate) trait Host {
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Failure>;
}
fn decode<T: DeserializeOwned>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value)
        .map_err(|e| Failure::Protocol(format!("malformed host fact: {e}")))
}
fn read(
    host: &mut impl Host,
    evaluation: &str,
    queries: Vec<Value>,
) -> Result<Vec<Value>, Failure> {
    if queries.len() > MAX_BATCH {
        return Err(Failure::Protocol("fact batch limit".into()));
    }
    let count = queries.len();
    let response = host.request(
        "analysis/readFacts",
        json!({"evaluation":evaluation,"queries":queries}),
    )?;
    let facts = response["facts"]
        .as_array()
        .ok_or_else(|| Failure::Protocol("missing host facts".into()))?;
    if facts.len() != count {
        return Err(Failure::Protocol("host fact count mismatch".into()));
    }
    Ok(facts.clone())
}
fn type_fact(value: Value, registry: &mut Registry) -> Result<Fact<TypeFact>, Failure> {
    let fact: Fact<TypeFact> = decode(value)?;
    fact.validate()?;
    if let Some(ty) = fact.found() {
        registry.register(&ty.id, HandleKind::Type)?;
        if ty.qualified_name.is_empty() {
            return Err(Failure::Protocol("empty host qualified name".into()));
        }
    }
    Ok(fact)
}
fn hint(output: &mut Contributions, range: ByteRange, message: &str) {
    output.diagnostics.push(Diagnostic {
        range,
        severity: "hint".into(),
        code: None,
        message: format!("[mcdev.mixin.incomplete] {message}"),
    });
}
fn target_values<'a>(
    argument: &'a Argument,
    strings: bool,
    values: &mut Vec<&'a AnnotationValue>,
) -> bool {
    match argument.value.found() {
        Some(AnnotationValue::Array { values: items }) => {
            items.iter().all(|a| target_values(a, strings, values))
        }
        Some(value @ AnnotationValue::Class { .. }) if !strings => {
            values.push(value);
            true
        }
        Some(value @ AnnotationValue::String { .. }) if strings => {
            values.push(value);
            true
        }
        _ => false,
    }
}
fn signature(member: &Member, registry: &mut Registry) -> Result<(), Failure> {
    registry.register(&member.id, HandleKind::Member)?;
    member.signature.validate()?;
    member.dispatch.validate()?;
    if !matches!(member.kind.as_str(), "field" | "method") {
        return Err(Failure::Protocol("invalid host member kind".into()));
    }
    if let Some(sig) = member.signature.found() {
        registry.register(&sig.id, HandleKind::Signature)?;
        if sig.kind != member.kind
            || if sig.kind == "method" {
                parse_method_descriptor(&sig.descriptor).is_err()
            } else {
                parse_field_descriptor(&sig.descriptor).is_err()
            }
        {
            return Err(Failure::Protocol("invalid host signature".into()));
        }
    }
    if member
        .dispatch
        .found()
        .is_some_and(|d| !matches!(d.as_str(), "static" | "instance"))
    {
        return Err(Failure::Protocol("invalid host dispatch".into()));
    }
    Ok(())
}

pub(crate) fn analyze(e: &Evaluation, host: &mut impl Host) -> Result<Contributions, Failure> {
    let mut registry = Registry::default();
    e.validate(&mut registry)?;
    let mut output = Contributions::default();
    let mut pending = Vec::new();
    let annotations = e
        .declarations
        .iter()
        .flat_map(|d| &d.annotations)
        .collect::<Vec<_>>();
    let mut identities = BTreeMap::new();
    for batch in annotations.chunks(MAX_BATCH) {
        let queries = batch
            .iter()
            .map(|a| json!({"op":"resolveTypeRef","reference":a.type_ref}))
            .collect();
        for (annotation, value) in batch.iter().zip(read(host, &e.evaluation, queries)?) {
            let fact = type_fact(value, &mut registry)?;
            let identity = fact.found().and_then(|ty| {
                MixinAnnotation::from_official_fqn(&ty.qualified_name)
                    .filter(|a| a.official_fqns().first() == Some(&ty.qualified_name))
            });
            identities.insert(annotation.type_ref.clone(), identity);
        }
    }
    for owner in &e.declarations {
        let Some(mixin) = owner.annotations.iter().find(|a| {
            identities.get(&a.type_ref) == Some(&Some(MixinAnnotation::Mixin))
                && a.use_site_target.is_none()
        }) else {
            continue;
        };
        let mut values = vec![];
        let known = mixin
            .arguments
            .iter()
            .filter(|a| matches!(a.name.as_deref(), None | Some("value" | "targets")))
            .all(|a| target_values(a, a.name.as_deref() == Some("targets"), &mut values));
        if !known || values.len() != 1 {
            hint(
                &mut output,
                mixin.range.clone(),
                "exactly one known target is supported",
            );
            continue;
        }
        let target_query = match values[0] {
            AnnotationValue::Class { type_ref } => {
                registry.require(type_ref, HandleKind::TypeRef)?;
                json!({"op":"resolveTypeRef","reference":type_ref})
            }
            AnnotationValue::String { value, .. } => json!({"op":"resolveType","name":value}),
            _ => unreachable!(),
        };
        let target = type_fact(
            read(host, &e.evaluation, vec![target_query])?.remove(0),
            &mut registry,
        )?;
        let declarations = e
            .declarations
            .iter()
            .filter(|d| {
                d.owner.as_ref() == Some(&owner.id)
                    && d.annotations.iter().any(|a| {
                        identities.get(&a.type_ref) == Some(&Some(MixinAnnotation::Shadow))
                    })
            })
            .collect::<Vec<_>>();
        let mut authored_members = BTreeMap::new();
        for batch in declarations.chunks(MAX_BATCH) {
            let queries = batch
                .iter()
                .map(|d| json!({"op":"declarationMember","declaration":d.id}))
                .collect();
            for (declaration, value) in batch.iter().zip(read(host, &e.evaluation, queries)?) {
                let fact: Fact<Member> = decode(value)?;
                fact.validate()?;
                if let Some(member) = fact.found() {
                    signature(member, &mut registry)?;
                }
                authored_members.insert(declaration.id.clone(), fact);
            }
        }
        for declaration in declarations {
            let Some(shadow) = declaration
                .annotations
                .iter()
                .find(|a| identities.get(&a.type_ref) == Some(&Some(MixinAnnotation::Shadow)))
            else {
                continue;
            };
            // The host selects this exact declaration. Never reconstruct its identity by name.
            let Some(member) = authored_members[&declaration.id].found() else {
                hint(
                    &mut output,
                    declaration.range.clone(),
                    "authored member facts are incomplete",
                );
                continue;
            };
            if !matches!(shadow.use_site_target.as_deref(), None | Some("field"))
                || shadow.use_site_target.is_some() && member.kind != "field"
            {
                hint(
                    &mut output,
                    declaration.range.clone(),
                    "accessor use-site targets are not supported",
                );
                continue;
            }
            if shadow.arguments.iter().any(|a|a.name.as_deref()==Some("aliases")&&!matches!(a.value.found(),Some(AnnotationValue::Array{values}) if values.is_empty())){
                hint(&mut output,declaration.range.clone(),"Shadow aliases are not supported");continue;
            }
            let mut prefix = "shadow$";
            if let Some(a) = shadow
                .arguments
                .iter()
                .find(|a| a.name.as_deref() == Some("prefix"))
            {
                let Some(AnnotationValue::String { value, .. }) = a.value.found() else {
                    hint(
                        &mut output,
                        declaration.range.clone(),
                        "Shadow prefix is not a known string",
                    );
                    continue;
                };
                prefix = value;
            }
            let spelling = declaration.range.text(&e.source)?;
            let quoted = spelling.starts_with('`') && spelling.ends_with('`');
            let mut editable = ByteRange {
                start: declaration.range.start + usize::from(quoted),
                end: declaration.range.end - usize::from(quoted),
            };
            let authored = editable.text(&e.source)?;
            if member.name != authored {
                return Err(Failure::Protocol(
                    "host authored member spelling mismatch".into(),
                ));
            }
            if member.kind == "field" && authored.starts_with(prefix) {
                output.diagnostics.push(Diagnostic{range:declaration.range.clone(),severity:"error".into(),code:None,message:"[mcdev.mixin.shadow.field-prefix] Shadow fields cannot use the method prefix".into()});
                continue;
            }
            let (name, link) = if member.kind == "method" && !prefix.is_empty() {
                if let Some(name) = authored.strip_prefix(prefix) {
                    editable.start += prefix.len();
                    (name, json!({"kind":"prefixed","prefix":prefix}))
                } else {
                    (authored, json!({"kind":"direct"}))
                }
            } else {
                (authored, json!({"kind":"direct"}))
            };
            if name.is_empty() {
                hint(
                    &mut output,
                    declaration.range.clone(),
                    "Shadow prefix leaves an empty target name",
                );
                continue;
            }
            pending.push((json!({"target":target,"authored":declaration.id,"names":{"kind":"exact","values":[name]},"kind":member.kind,"signature":member.signature,"dispatch":member.dispatch,"nameLink":link}),declaration.range.clone(),editable));
        }
    }
    for batch in pending.chunks(MAX_BATCH) {
        let response=host.request("analysis/require",json!({"evaluation":e.evaluation,"requirements":batch.iter().map(|(proposal,_,_)|proposal).collect::<Vec<_>>()}))?;
        let requirements = response["requirements"]
            .as_array()
            .filter(|a| a.len() == batch.len())
            .ok_or_else(|| Failure::Protocol("requirement result count mismatch".into()))?;
        for (requirement, (_, full_range, editable)) in requirements.iter().zip(batch) {
            let id = requirement["id"]
                .as_str()
                .ok_or_else(|| Failure::Protocol("missing requirement handle".into()))?;
            registry.register(id, HandleKind::Requirement)?;
            let binding: Fact<Value> = decode(requirement["binding"].clone())?;
            binding.validate()?;
            if let Some(binding) = binding.found() {
                let target = binding["target"]
                    .as_str()
                    .ok_or_else(|| Failure::Protocol("missing bound member handle".into()))?;
                registry.register(target, HandleKind::Member)?;
            }
            output.sites.push(Site {
                requirement: id.into(),
                full_range: full_range.clone(),
                editable_name: editable.clone(),
                rename_subject: "target".into(),
            });
            match binding {
                Fact::Found { .. } => (),
                Fact::Unknown { .. } => hint(
                    &mut output,
                    full_range.clone(),
                    "Shadow target binding is incomplete",
                ),
                Fact::DefinitelyAbsent => output.diagnostics.push(Diagnostic {
                    range: full_range.clone(),
                    severity: "error".into(),
                    code: None,
                    message: "[mcdev.mixin.shadow.missing] no matching Shadow target member".into(),
                }),
            }
        }
    }
    if output.sites.len() + output.diagnostics.len() > MAX_HANDLES {
        return Err(Failure::Protocol("contribution limit".into()));
    }
    Ok(output)
}
