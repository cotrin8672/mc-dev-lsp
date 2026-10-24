use super::*;
use serde::{Deserialize, Serialize};

pub const INJECTION_POINT_SPECIFIER_FQN: &str =
    "org.spongepowered.asm.mixin.injection.InjectionPoint$Specifier";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInjectionPointSubtype {
    pub qualified_name: String,
    pub annotations: Vec<HostAnnotationFact>,
}
pub trait HostInjectionPointProvider: HostReadValidator {
    fn specifier_exists(&self, project: &HostProjectKey) -> HostSourceResult<bool>;
    /// Host-selected descendants of InjectionPoint, preserving declared annotation facts.
    fn injection_point_subtypes(
        &self,
        project: &HostProjectKey,
    ) -> HostSourceResult<Vec<HostInjectionPointSubtype>>;
}
pub struct InjectionPointFeatureProbe {
    cache: HostFactCache<HostProjectKey, bool>,
}
impl Default for InjectionPointFeatureProbe {
    fn default() -> Self {
        Self::new(32)
    }
}
impl InjectionPointFeatureProbe {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: HostFactCache::new(capacity, None),
        }
    }
    pub fn supports_specifier(
        &self,
        project: Option<&HostProjectKey>,
        provider: &dyn HostInjectionPointProvider,
    ) -> Result<CachedHostAnswer<bool>, HostSourceError> {
        let Some(project) = project else {
            return Ok(CachedHostAnswer {
                answer: HostAnswer::unavailable("project semantic facts unavailable"),
                cache_hit: false,
            });
        };
        self.cache
            .get_or_compute(project.clone(), project.generations, provider, || {
                provider.specifier_exists(project)
            })
    }
    pub fn clear(&self) {
        self.cache.clear();
    }
}
pub struct InjectionPointAtCodeIndex {
    cache: HostFactCache<HostProjectKey, Vec<String>>,
}
impl Default for InjectionPointAtCodeIndex {
    fn default() -> Self {
        Self::new(32)
    }
}
impl InjectionPointAtCodeIndex {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: HostFactCache::new(capacity, None),
        }
    }
    pub fn values(
        &self,
        project: Option<&HostProjectKey>,
        provider: &dyn HostInjectionPointProvider,
    ) -> Result<CachedHostAnswer<Vec<String>>, HostSourceError> {
        let Some(project) = project else {
            return Ok(CachedHostAnswer {
                answer: HostAnswer::unavailable("project semantic facts unavailable"),
                cache_hit: false,
            });
        };
        self.cache
            .get_or_compute(project.clone(), project.generations, provider, || {
                let answer = provider.injection_point_subtypes(project)?;
                Ok(HostAnswer {
                    fact: match answer.fact {
                        HostFact::Known(subtypes) => {
                            HostFact::Known(injection_point_at_codes(&subtypes))
                        }
                        HostFact::Missing => HostFact::Known(vec![]),
                        HostFact::Unavailable { reason } => HostFact::Unavailable { reason },
                    },
                    reads: answer.reads,
                })
            })
    }
    pub fn clear(&self) {
        self.cache.clear();
    }
}
pub fn injection_point_at_codes(subtypes: &[HostInjectionPointSubtype]) -> Vec<String> {
    let mut codes = std::collections::BTreeSet::new();
    for subtype in subtypes {
        if let Some(code) = subtype.annotations.iter().find_map(declared_at_code) {
            codes.insert(code);
        } else if !subtype
            .qualified_name
            .starts_with("org.spongepowered.asm.mixin.")
            && !subtype.qualified_name.trim().is_empty()
        {
            codes.insert(subtype.qualified_name.clone());
        }
    }
    codes.into_iter().collect()
}
pub fn declared_at_code(annotation: &HostAnnotationFact) -> Option<String> {
    let name = annotation
        .qualified_name
        .as_deref()
        .unwrap_or(&annotation.source_name);
    if ![
        "AtCode",
        "org.spongepowered.asm.mixin.injection.InjectionPoint$AtCode",
        "org.spongepowered.asm.mixin.injection.InjectionPoint.AtCode",
    ]
    .contains(&name)
        || !annotation.validity.is_usable()
        || !annotation.exists
    {
        return None;
    }
    let mut value = None;
    let mut namespace = None;
    for (name, node) in &annotation.members {
        match name.as_str() {
            "value" => value = node.constant_string(),
            "namespace" => namespace = node.constant_string(),
            _ => {}
        }
    }
    let value = value.filter(|v| !v.trim().is_empty())?;
    Some(format_at_code_value(namespace.as_deref(), &value))
}
pub fn format_at_code_value(namespace: Option<&str>, value: &str) -> String {
    match namespace.filter(|v| !v.trim().is_empty()) {
        Some(namespace) => format!("{namespace}:{value}"),
        None => value.into(),
    }
}
