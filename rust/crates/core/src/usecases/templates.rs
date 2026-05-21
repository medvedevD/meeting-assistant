//! Template CRUD use-cases.
//!
//! The template **name is also its file name** on disk, so every mutating
//! operation funnels through [`validate_template_name`] first. The validation
//! is a strict whitelist (decision #8 in the legacy-feature-port plan): Unicode
//! letters/digits, space, `-` and `_`, at most 100 chars, and the name must
//! survive sanitisation unchanged. That last rule is what blocks path traversal
//! (`../../evil`, `a/b`, `C:\x`) and other surprises — any character that is not
//! on the whitelist makes `sanitized != input`, so the name is rejected rather
//! than silently rewritten. Existing Cyrillic names (`1-на-1`, `Дейлик`) pass
//! because `char::is_alphanumeric` is Unicode-aware.

use std::sync::Arc;

use crate::ports::TemplateLoader;
use crate::CoreError;

/// Maximum template-name length, in `char`s (not bytes).
const MAX_NAME_LEN: usize = 100;

/// A single template plus its body, for the list+preview view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateWithBody {
    pub name: String,
    pub body: String,
}

/// True for characters allowed in a template name: any Unicode letter or digit,
/// plus space, hyphen and underscore.
fn is_allowed(c: char) -> bool {
    c.is_alphanumeric() || c == ' ' || c == '-' || c == '_'
}

/// Validate a template name. Returns [`CoreError::Validation`] on rejection.
///
/// Rejects empty names, names longer than [`MAX_NAME_LEN`] chars, and any name
/// that contains a character outside the whitelist (which covers path
/// separators, `.`, `..`, NUL, control chars, etc.).
pub fn validate_template_name(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        return Err(CoreError::Validation("template name must not be empty".into()));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(CoreError::Validation(format!(
            "template name must be at most {MAX_NAME_LEN} characters"
        )));
    }
    let sanitized: String = name.chars().filter(|&c| is_allowed(c)).collect();
    if sanitized != name {
        return Err(CoreError::Validation(
            "template name may contain only letters, digits, spaces, '-' and '_'".into(),
        ));
    }
    Ok(())
}

/// List every template together with its body (for previews).
pub async fn list_templates(
    loader: &Arc<dyn TemplateLoader>,
) -> Result<Vec<TemplateWithBody>, CoreError> {
    let names = loader.list_names().await?;
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let body = loader.load(&name).await?.unwrap_or_default();
        out.push(TemplateWithBody { name, body });
    }
    Ok(out)
}

/// Load a single template body. Returns [`CoreError::NotFound`] if absent.
pub async fn get_template(
    loader: &Arc<dyn TemplateLoader>,
    name: &str,
) -> Result<String, CoreError> {
    validate_template_name(name)?;
    loader
        .load(name)
        .await?
        .ok_or_else(|| CoreError::NotFound(format!("template '{name}'")))
}

/// Create or overwrite a template after validating its name.
pub async fn save_template(
    loader: &Arc<dyn TemplateLoader>,
    name: &str,
    body: &str,
) -> Result<(), CoreError> {
    validate_template_name(name)?;
    loader.save(name, body).await
}

/// Delete a template after validating its name.
pub async fn delete_template(
    loader: &Arc<dyn TemplateLoader>,
    name: &str,
) -> Result<(), CoreError> {
    validate_template_name(name)?;
    loader.delete(name).await
}

/// Rename a template. Both names are validated; the new name must be free.
pub async fn rename_template(
    loader: &Arc<dyn TemplateLoader>,
    old: &str,
    new: &str,
) -> Result<(), CoreError> {
    validate_template_name(old)?;
    validate_template_name(new)?;
    if old == new {
        return Ok(());
    }
    if loader.load(new).await?.is_some() {
        return Err(CoreError::AlreadyExists(format!("template '{new}'")));
    }
    loader.rename(old, new).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeTemplateLoader;

    #[test]
    fn accepts_existing_cyrillic_names() {
        validate_template_name("1-на-1").unwrap();
        validate_template_name("Дейлик").unwrap();
        validate_template_name("Weekly sync_2").unwrap();
    }

    #[test]
    fn rejects_path_traversal_and_separators() {
        for bad in ["../../evil", "a/b", "..", ".", "x\\y", "with.dot", "tab\there", ""] {
            assert!(
                validate_template_name(bad).is_err(),
                "expected '{bad}' to be rejected"
            );
        }
    }

    #[test]
    fn rejects_overlong_names() {
        let long = "a".repeat(101);
        assert!(validate_template_name(&long).is_err());
        let max = "a".repeat(100);
        assert!(validate_template_name(&max).is_ok());
    }

    #[tokio::test]
    async fn rename_to_existing_name_conflicts() {
        let loader: Arc<dyn TemplateLoader> =
            FakeTemplateLoader::new([("a", "body-a"), ("b", "body-b")]);
        let err = rename_template(&loader, "a", "b").await.unwrap_err();
        assert!(matches!(err, CoreError::AlreadyExists(_)));
    }

    #[tokio::test]
    async fn save_rejects_invalid_name_before_touching_loader() {
        let loader: Arc<dyn TemplateLoader> = FakeTemplateLoader::empty();
        let err = save_template(&loader, "../evil", "x").await.unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }
}
