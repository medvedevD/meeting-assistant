use std::sync::Arc;

use async_trait::async_trait;
use meeting_adapters::{EmbeddedBundle, JsonSettingsStore};
use meeting_api::{TemplateDto, TemplateError, TemplateService};
use meeting_core::ports::{TemplateBundle, TemplateLoader};
use meeting_core::usecases;
use meeting_core::CoreError;

/// True if `name` is one of the templates shipped with the app. Used to keep
/// `settings.removed_bundled_templates` (the startup-backfill tombstone list) in
/// sync with user edits: deleting a bundled template tombstones it; re-creating
/// it clears the tombstone.
fn is_bundled(name: &str) -> bool {
    EmbeddedBundle.entries().iter().any(|(n, _)| n == name)
}

/// Concrete [`TemplateService`] over the file-backed [`TemplateLoader`] and the
/// JSON settings store. Validation lives in the core use-cases; this layer maps
/// `CoreError` onto the wire error and owns the cross-cutting concern the core
/// can't see — clearing a dangling `default_template` when its template is
/// deleted (plan decision #7).
pub struct AppTemplateService {
    templates: Arc<dyn TemplateLoader>,
    settings_store: Arc<JsonSettingsStore>,
}

impl AppTemplateService {
    pub fn new(templates: Arc<dyn TemplateLoader>, settings_store: Arc<JsonSettingsStore>) -> Self {
        Self {
            templates,
            settings_store,
        }
    }
}

/// Map a core error onto the HTTP-facing template error.
fn map_err(e: CoreError) -> TemplateError {
    match e {
        CoreError::Validation(m) => TemplateError::Validation(m),
        CoreError::NotFound(m) => TemplateError::NotFound(m),
        CoreError::AlreadyExists(m) => TemplateError::Conflict(m),
        other => TemplateError::Internal(other.to_string()),
    }
}

#[async_trait]
impl TemplateService for AppTemplateService {
    async fn list(&self) -> Result<Vec<TemplateDto>, TemplateError> {
        let items = usecases::list_templates(&self.templates)
            .await
            .map_err(map_err)?;
        Ok(items
            .into_iter()
            .map(|t| TemplateDto {
                name: t.name,
                body: t.body,
            })
            .collect())
    }

    async fn get(&self, name: &str) -> Result<String, TemplateError> {
        usecases::get_template(&self.templates, name)
            .await
            .map_err(map_err)
    }

    async fn save(&self, name: &str, body: &str) -> Result<(), TemplateError> {
        usecases::save_template(&self.templates, name, body)
            .await
            .map_err(map_err)?;

        // Re-creating a bundled template the user had deleted clears its
        // tombstone, so future backfills treat it as present again.
        if is_bundled(name) {
            let mut settings = self.settings_store.load();
            if let Some(pos) = settings
                .removed_bundled_templates
                .iter()
                .position(|n| n == name)
            {
                settings.removed_bundled_templates.remove(pos);
                let _ = self.settings_store.save(settings);
            }
        }
        Ok(())
    }

    async fn delete(&self, name: &str) -> Result<Option<String>, TemplateError> {
        usecases::delete_template(&self.templates, name)
            .await
            .map_err(map_err)?;

        let mut settings = self.settings_store.load();
        let mut dirty = false;

        // Tombstone a deleted bundled template so startup backfill won't
        // resurrect it on the next launch.
        if is_bundled(name) && !settings.removed_bundled_templates.iter().any(|n| n == name) {
            settings.removed_bundled_templates.push(name.to_string());
            dirty = true;
        }

        // Decision #7: if the deleted template was the configured default, clear
        // the reference (the app falls back to the built-in default) and tell
        // the client so it can surface a toast.
        let mut notice = None;
        if settings.default_template.as_deref() == Some(name) {
            settings.default_template = None;
            dirty = true;
            notice = Some(format!(
                "Шаблон «{name}» был выбран по умолчанию — сброшено на встроенный шаблон."
            ));
        }

        if dirty {
            if let Err(e) = self.settings_store.save(settings) {
                // The template is already gone; a failed settings write is
                // non-fatal but worth flagging rather than silently swallowing.
                return Ok(Some(format!(
                    "Шаблон удалён, но не удалось сохранить настройки: {e}"
                )));
            }
        }
        Ok(notice)
    }

    async fn rename(&self, old: &str, new: &str) -> Result<(), TemplateError> {
        usecases::rename_template(&self.templates, old, new)
            .await
            .map_err(map_err)?;

        let mut settings = self.settings_store.load();
        let mut dirty = false;

        // Keep `default_template` pointing at the renamed file so the user's
        // default is not silently lost.
        if settings.default_template.as_deref() == Some(old) {
            settings.default_template = Some(new.to_string());
            dirty = true;
        }

        // Renaming a bundled template away tombstones the old name (else backfill
        // would re-seed it); renaming onto a bundled name clears that tombstone.
        if old != new {
            if is_bundled(old) && !settings.removed_bundled_templates.iter().any(|n| n == old) {
                settings.removed_bundled_templates.push(old.to_string());
                dirty = true;
            }
            if let Some(pos) = settings
                .removed_bundled_templates
                .iter()
                .position(|n| n == new)
            {
                settings.removed_bundled_templates.remove(pos);
                dirty = true;
            }
        }

        if dirty {
            let _ = self.settings_store.save(settings);
        }
        Ok(())
    }
}
