//! Positive annotation sites from one accepted source view, including unowned syntax.
use super::*;
use crate::wire::{
    validate_annotation, Annotation, AnnotationValue, Argument, Declaration, HandleKind, Registry,
    TypeFact, MAX_HANDLES,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationSource {
    pub document_uri: Fact<String>,
    pub identity: InputIdentity,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationSite {
    #[serde(deserialize_with = "nullable_owner")]
    pub owner: Option<String>,
    #[serde(flatten)]
    pub annotation: Annotation,
}

fn nullable_owner<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationSites {
    pub source: AnnotationSource,
    pub annotations: Vec<AnnotationSite>,
    pub coverage: Fact<()>,
}

impl AnnotationSites {
    fn validate_shape(&self) -> Result<(), FactsError> {
        validate_fact(&self.source.document_uri)?;
        validate_fact(&self.coverage)?;
        if !valid_identity(&self.source.identity, "source")
            || !valid_digest(&self.source.sha256)
            || self.annotations.len() > MAX_HANDLES
            || matches!(self.source.document_uri, Fact::DefinitelyAbsent)
            || matches!(self.coverage, Fact::DefinitelyAbsent)
        {
            return Err(protocol("invalid annotation site footprint or coverage"));
        }
        if let Some(uri) = self.source.document_uri.found() {
            if !uri.starts_with("file://") || mcdev_core::project::uri_to_path(uri).is_err() {
                return Err(protocol("invalid annotation source URI"));
            }
        }
        Ok(())
    }

    /// Return only genuinely unowned top-level sites. Parameter/nested sites are
    /// reconciled first; a containing class never becomes their declaration owner.
    pub fn validate<'a>(
        &'a self,
        text: &str,
        uri: &str,
        expected: Option<&SourceDescriptor>,
        declarations: &[Declaration],
        detailed: bool,
        registry: &mut Registry,
    ) -> Result<Vec<&'a Annotation>, FactsError> {
        self.reconcile(text, uri, expected, declarations, detailed, registry, None)
    }

    /// Resolve both reference sets in this evaluation before comparing aliases.
    /// The returned Type IDs are not canonical declaration identities either.
    pub fn validate_with_types<'a>(
        &'a self,
        text: &str,
        uri: &str,
        expected: Option<&SourceDescriptor>,
        declarations: &[Declaration],
        detailed: bool,
        registry: &mut Registry,
        types: &BTreeMap<String, Fact<TypeFact>>,
    ) -> Result<Vec<&'a Annotation>, FactsError> {
        self.reconcile(
            text,
            uri,
            expected,
            declarations,
            detailed,
            registry,
            Some(types),
        )
    }

    pub(crate) fn validate_source(
        &self,
        text: &str,
        uri: &str,
        expected: Option<&SourceDescriptor>,
    ) -> Result<(), FactsError> {
        self.validate_shape()?;
        let actual_uri =
            self.source.document_uri.found().ok_or_else(|| {
                FactsError::Unknown("annotation source URI is unavailable".into())
            })?;
        if !same_file_uri(actual_uri, uri)? || !digest_matches(text.as_bytes(), &self.source.sha256)
        {
            return Err(FactsError::Stale(
                "annotation source URI or digest mismatch".into(),
            ));
        }
        if let Some(expected) = expected {
            let expected_uri = expected.document_uri.found().ok_or_else(|| {
                FactsError::Unknown("expected annotation source URI is unavailable".into())
            })?;
            if self.source.identity != expected.identity
                || !same_file_uri(actual_uri, expected_uri)?
            {
                return Err(FactsError::Stale(
                    "annotation source snapshot identity mismatch".into(),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_values(
        &self,
        text: &str,
        registry: &mut Registry,
    ) -> Result<(), FactsError> {
        for site in &self.annotations {
            let mut nodes = 0;
            validate_annotation(&site.annotation, text, registry, 0, true, &mut nodes)
                .map_err(protocol)?;
            if let Some(owner) = &site.owner {
                registry
                    .require(owner, HandleKind::Declaration)
                    .map_err(protocol)?;
            }
        }
        Ok(())
    }

    fn reconcile<'a>(
        &'a self,
        text: &str,
        uri: &str,
        expected: Option<&SourceDescriptor>,
        declarations: &[Declaration],
        detailed: bool,
        registry: &mut Registry,
        types: Option<&BTreeMap<String, Fact<TypeFact>>>,
    ) -> Result<Vec<&'a Annotation>, FactsError> {
        self.validate_source(text, uri, expected)?;
        self.validate_values(text, registry)?;
        let mut known = BTreeMap::new();
        let mut parameters = Vec::new();
        for declaration in declarations {
            registry
                .require(&declaration.id, HandleKind::Declaration)
                .map_err(protocol)?;
            // A recovered/unsupported parameter list grants no unowned injector
            // authority to annotations inside an already authored callable header.
            if declaration.source_kind.as_deref() == Some("Function") {
                if let Some(header) = &declaration.header_range {
                    parameters.push(header);
                }
            }
            for annotation in &declaration.annotations {
                known_annotation(annotation, Some(declaration.id.as_str()), &mut known)?;
            }
            for parameter in declaration
                .parameters
                .as_ref()
                .and_then(Fact::found)
                .into_iter()
                .flatten()
            {
                parameters.push(&parameter.range);
                for annotation in &parameter.annotations {
                    known_annotation(annotation, None, &mut known)?;
                }
            }
        }
        let mut returned = BTreeSet::new();
        let mut all: BTreeMap<_, _> = known
            .iter()
            .map(|(range, (annotation, _))| (*range, *annotation))
            .collect();
        for site in &self.annotations {
            let annotation = &site.annotation;
            let key = (annotation.range.start, annotation.range.end);
            if !returned.insert(key) {
                return Err(protocol("duplicate annotation site range"));
            }
            match known.get(&key) {
                Some((previous, owner)) => {
                    if site.owner.as_deref() != *owner
                        || !compatible_annotation(previous, annotation, detailed, types)?
                    {
                        return Err(protocol(
                            "annotation site disagrees with its declared, parameter or nested site",
                        ));
                    }
                }
                None if site.owner.is_some() => {
                    return Err(protocol(
                        "annotation site has no matching declaration-owned annotation",
                    ));
                }
                None => (),
            }
            all.insert(key, annotation);
        }
        if self.coverage.found().is_some() && known.keys().any(|range| !returned.contains(range)) {
            return Err(protocol(
                "complete annotation inventory omitted a known annotation site",
            ));
        }
        // A flat inventory may include a nested annotation and its outer annotation.
        // Only genuine argument containment permits such overlap.
        let mut ordered = all.values().copied().collect::<Vec<_>>();
        ordered.sort_by_key(|a| (a.range.start, std::cmp::Reverse(a.range.end)));
        let mut parents: Vec<&Annotation> = Vec::new();
        for annotation in &ordered {
            while parents
                .last()
                .is_some_and(|parent| parent.range.end <= annotation.range.start)
            {
                parents.pop();
            }
            if let Some(parent) = parents.last() {
                if !parent
                    .arguments
                    .iter()
                    .any(|argument| contains(&argument.range, &annotation.range))
                {
                    return Err(protocol(
                        "overlapping annotation sites are not nested arguments",
                    ));
                }
            }
            parents.push(annotation);
        }
        Ok(self
            .annotations
            .iter()
            .filter_map(|site| {
                let annotation = &site.annotation;
                if site.owner.is_some()
                    || known.contains_key(&(annotation.range.start, annotation.range.end))
                    || parameters
                        .iter()
                        .any(|range| contains(range, &annotation.range))
                    || ordered.iter().any(|outer| {
                        outer.range != annotation.range
                            && outer
                                .arguments
                                .iter()
                                .any(|argument| contains(&argument.range, &annotation.range))
                    })
                {
                    None
                } else {
                    Some(annotation)
                }
            })
            .collect())
    }
}

fn contains(outer: &crate::wire::ByteRange, inner: &crate::wire::ByteRange) -> bool {
    outer.start <= inner.start && inner.start <= inner.end && inner.end <= outer.end
}

fn same_file_uri(left: &str, right: &str) -> Result<bool, FactsError> {
    use mcdev_core::project::{normalize_path, uri_to_path};
    if !left.starts_with("file://") || !right.starts_with("file://") {
        return Err(protocol("annotation source must use file URI"));
    }
    let left = normalize_path(&uri_to_path(left).map_err(protocol)?);
    let right = normalize_path(&uri_to_path(right).map_err(protocol)?);
    Ok(if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    })
}

fn known_annotation<'a>(
    annotation: &'a Annotation,
    owner: Option<&'a str>,
    known: &mut BTreeMap<(usize, usize), (&'a Annotation, Option<&'a str>)>,
) -> Result<(), FactsError> {
    if known
        .insert(
            (annotation.range.start, annotation.range.end),
            (annotation, owner),
        )
        .is_some()
    {
        return Err(protocol(
            "duplicate declaration, parameter or nested annotation range",
        ));
    }
    for argument in &annotation.arguments {
        known_argument(argument, known)?;
    }
    Ok(())
}

fn known_argument<'a>(
    argument: &'a Argument,
    known: &mut BTreeMap<(usize, usize), (&'a Annotation, Option<&'a str>)>,
) -> Result<(), FactsError> {
    match argument.value.found() {
        Some(AnnotationValue::Annotation { annotation }) => {
            known_annotation(annotation, None, known)?
        }
        Some(AnnotationValue::Array { values }) => {
            for value in values {
                known_argument(value, known)?;
            }
        }
        _ => (),
    }
    Ok(())
}

fn compatible_annotation(
    previous: &Annotation,
    next: &Annotation,
    detailed: bool,
    types: Option<&BTreeMap<String, Fact<TypeFact>>>,
) -> Result<bool, FactsError> {
    if previous.range != next.range
        || previous.use_site_target != next.use_site_target
        || previous.arguments.len() != next.arguments.len()
        || if detailed {
            previous.name_range != next.name_range
        } else {
            previous
                .name_range
                .as_ref()
                .is_some_and(|range| Some(range) != next.name_range.as_ref())
        }
        || !compatible_reference(&previous.type_ref, &next.type_ref, types)?
    {
        return Ok(false);
    }
    for (a, b) in previous.arguments.iter().zip(&next.arguments) {
        if !compatible_argument(a, b, detailed, types)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn compatible_reference(
    a: &str,
    b: &str,
    types: Option<&BTreeMap<String, Fact<TypeFact>>>,
) -> Result<bool, FactsError> {
    if a == b {
        return Ok(true);
    }
    let Some(types) = types else {
        return Ok(false);
    };
    let resolved = |reference: &str| {
        types.get(reference).and_then(Fact::found).ok_or_else(|| {
            FactsError::Unknown(
                "annotation reference alias has no positive same-view identity".into(),
            )
        })
    };
    let a = resolved(a)?;
    let b = resolved(b)?;
    if a.qualified_name.is_empty() || b.qualified_name.is_empty() {
        return Err(protocol("annotation type identity is empty"));
    }
    if a.qualified_name != b.qualified_name || a.read_only != b.read_only {
        return Ok(false);
    }
    // Type and source handle strings can be re-issued. Provenance remains exact.
    Ok(match (&a.context_source, &b.context_source, a.read_only) {
        (None, None, false) => true,
        (Some(a), Some(b), true) => {
            a.identity == b.identity
                && a.language_id == b.language_id
                && match (a.document_uri.found(), b.document_uri.found()) {
                    (Some(a), Some(b)) => same_file_uri(a, b)?,
                    _ => false,
                }
        }
        _ => false,
    })
}

fn compatible_argument(
    a: &Argument,
    b: &Argument,
    detailed: bool,
    types: Option<&BTreeMap<String, Fact<TypeFact>>>,
) -> Result<bool, FactsError> {
    if a.name != b.name || a.range != b.range {
        return Ok(false);
    }
    Ok(match (&a.value, &b.value) {
        (Fact::Unknown { .. }, _) if !detailed => true,
        (
            Fact::Found {
                value: AnnotationValue::Class { type_ref: a },
            },
            Fact::Found {
                value: AnnotationValue::Class { type_ref: b },
            },
        ) => compatible_reference(a, b, types)?,
        (
            Fact::Found {
                value:
                    AnnotationValue::Enum {
                        type_ref: a,
                        name: an,
                    },
            },
            Fact::Found {
                value:
                    AnnotationValue::Enum {
                        type_ref: b,
                        name: bn,
                    },
            },
        ) => an == bn && compatible_reference(a, b, types)?,
        (
            Fact::Found {
                value: AnnotationValue::Annotation { annotation: a },
            },
            Fact::Found {
                value: AnnotationValue::Annotation { annotation: b },
            },
        ) => compatible_annotation(a, b, detailed, types)?,
        (
            Fact::Found {
                value: AnnotationValue::Array { values: a },
            },
            Fact::Found {
                value: AnnotationValue::Array { values: b },
            },
        ) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                if !compatible_argument(a, b, detailed, types)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => a.value == b.value,
    })
}

impl<R: FactReader> ScopedInputs<R> {
    pub fn annotation_sites(
        &mut self,
        source: Option<&SourceEntry>,
    ) -> Result<AnnotationSites, FactsError> {
        let mut query = json!({"op":"annotationSites"});
        if let Some(source) = source {
            check_entry(
                source.scope,
                self.scope,
                &source.descriptor.id,
                &source.descriptor,
                &self.sources,
            )?;
            query["source"] = json!(source.descriptor.id);
        }
        let sites: AnnotationSites = self.one(InputOp::AnnotationSites, query)?;
        sites.validate_shape()?;
        if let Some(source) = source {
            let same_uri = match (
                sites.source.document_uri.found(),
                source.descriptor.document_uri.found(),
            ) {
                (Some(actual), Some(expected)) => same_file_uri(actual, expected)?,
                _ => {
                    return Err(FactsError::Unknown(
                        "foreign annotation source URI is unavailable".into(),
                    ))
                }
            };
            if sites.source.identity != source.descriptor.identity || !same_uri {
                return Err(FactsError::Stale(
                    "annotation site reader returned a foreign snapshot".into(),
                ));
            }
        }
        Ok(sites)
    }
}
