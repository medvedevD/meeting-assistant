use std::path::PathBuf;
use async_trait::async_trait;
use meeting_core::{CoreError, ports::TemplateLoader};

/// Loads templates from `.md` files in a directory.
/// File name (without extension) is the template name.
pub struct FileTemplateLoader {
    dir: PathBuf,
}

impl FileTemplateLoader {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

#[async_trait]
impl TemplateLoader for FileTemplateLoader {
    async fn load(&self, name: &str) -> Result<Option<String>, CoreError> {
        let path = self.dir.join(format!("{name}.md"));
        if !path.exists() {
            return Ok(None);
        }
        tokio::fs::read_to_string(&path)
            .await
            .map(Some)
            .map_err(|e| CoreError::Template(e.to_string()))
    }

    async fn list_names(&self) -> Result<Vec<String>, CoreError> {
        let mut entries = match tokio::fs::read_dir(&self.dir).await {
            Ok(entries) => entries,
            // A missing prompts dir is a misconfiguration, not a fatal error:
            // return no templates so the UI (e.g. the settings window) stays usable.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("prompts dir not found: {} — no templates available", self.dir.display());
                return Ok(Vec::new());
            }
            Err(e) => return Err(CoreError::Template(e.to_string())),
        };

        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| CoreError::Template(e.to_string()))? {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[tokio::test]
    async fn loads_existing_template() {
        let tmp = setup();
        fs::write(tmp.path().join("Дейлик.md"), "Сделай протокол дейлика.").unwrap();

        let loader = FileTemplateLoader::new(tmp.path());
        let content = loader.load("Дейлик").await.unwrap();
        assert_eq!(content.unwrap(), "Сделай протокол дейлика.");
    }

    #[tokio::test]
    async fn returns_none_for_missing_template() {
        let tmp = setup();
        let loader = FileTemplateLoader::new(tmp.path());
        let result = loader.load("НеСуществует").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_names_returns_empty_when_dir_missing() {
        let loader = FileTemplateLoader::new("/no/such/prompts/dir");
        assert_eq!(loader.list_names().await.unwrap(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn lists_all_template_names() {
        let tmp = setup();
        fs::write(tmp.path().join("1-на-1.md"), "tmpl1").unwrap();
        fs::write(tmp.path().join("Дейлик.md"), "tmpl2").unwrap();
        fs::write(tmp.path().join("notes.txt"), "not a template").unwrap();

        let loader = FileTemplateLoader::new(tmp.path());
        let mut names = loader.list_names().await.unwrap();
        names.sort();

        assert!(names.contains(&"1-на-1".to_string()));
        assert!(names.contains(&"Дейлик".to_string()));
        assert!(!names.contains(&"notes".to_string()));
    }
}
