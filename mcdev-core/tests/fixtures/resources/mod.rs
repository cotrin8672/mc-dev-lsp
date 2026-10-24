//! Test-only named resources. No runtime scanner, JVM, or Gradle is required.
mod catalog;
pub fn bytes(path: &str) -> &'static [u8] {
    catalog::RESOURCES
        .iter()
        .find(|(name, _)| *name == path)
        .unwrap_or_else(|| panic!("missing fixture {path}"))
        .1
}
pub fn text(path: &str) -> &'static str {
    std::str::from_utf8(bytes(path)).unwrap()
}
pub fn list(root: &str) -> Vec<&'static str> {
    let prefix = format!("{root}/");
    catalog::RESOURCES
        .iter()
        .filter(|(name, _)| name.starts_with(&prefix))
        .map(|(name, _)| *name)
        .collect()
}
pub fn config(path: &str) -> serde_json::Value {
    serde_json::from_str(text(path)).unwrap()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureMixinConfig {
    pub required: bool,
    pub min_version: String,
    pub package_name: String,
    pub compatibility_level: String,
    pub mixins: Vec<String>,
}
pub fn parsed_config(path: &str) -> Result<FixtureMixinConfig, String> {
    let value = config(path);
    let string = |key: &str| {
        value
            .get(key)
            .ok_or_else(|| format!("missing required key '{key}'"))?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("'{key}' must be a string"))
    };
    let required = value
        .get("required")
        .ok_or("missing required key 'required'")?
        .as_bool()
        .ok_or("'required' must be a boolean")?;
    let mixins = value
        .get("mixins")
        .ok_or("missing required key 'mixins'")?
        .as_array()
        .ok_or("'mixins' must be an array")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "'mixins' entries must be strings".into())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(FixtureMixinConfig {
        required,
        min_version: string("minVersion")?,
        package_name: string("package")?,
        compatibility_level: string("compatibilityLevel")?,
        mixins,
    })
}
pub fn validate_config(value: &serde_json::Value) -> Vec<String> {
    let mut errors = vec![];
    for key in [
        "required",
        "minVersion",
        "package",
        "compatibilityLevel",
        "mixins",
    ] {
        if value.get(key).is_none() {
            errors.push(format!("missing required key '{key}'"));
        }
    }
    if value.get("mixins").is_some_and(|v| !v.is_array()) {
        errors.push("'mixins' must be an array".into())
    }
    errors
}
