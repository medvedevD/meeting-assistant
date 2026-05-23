use async_trait::async_trait;
use meeting_core::{ports::TemplateLoader, CoreError};
use std::path::PathBuf;
use std::sync::RwLock;

/// Loads templates from `.md` files in a directory.
/// File name (without extension) is the template name.
/// The directory is interior-mutable so it can be hot-swapped when the user
/// changes the prompts path in settings.
pub struct FileTemplateLoader {
    dir: RwLock<PathBuf>,
}

impl FileTemplateLoader {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: RwLock::new(dir.into()),
        }
    }

    fn dir(&self) -> PathBuf {
        self.dir.read().unwrap().clone()
    }

    /// Change the prompts directory at runtime.
    pub fn set_dir(&self, dir: impl Into<PathBuf>) {
        *self.dir.write().unwrap() = dir.into();
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.dir().join(format!("{name}.md"))
    }
}

#[async_trait]
impl TemplateLoader for FileTemplateLoader {
    async fn load(&self, name: &str) -> Result<Option<String>, CoreError> {
        let path = self.path_for(name);
        if !path.exists() {
            return Ok(None);
        }
        tokio::fs::read_to_string(&path)
            .await
            .map(Some)
            .map_err(|e| CoreError::Template(e.to_string()))
    }

    async fn list_names(&self) -> Result<Vec<String>, CoreError> {
        let dir = self.dir();
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            // A missing prompts dir is a misconfiguration, not a fatal error:
            // return no templates so the UI (e.g. the settings window) stays usable.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    "prompts dir not found: {} — no templates available",
                    dir.display()
                );
                return Ok(Vec::new());
            }
            Err(e) => return Err(CoreError::Template(e.to_string())),
        };

        let mut names = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| CoreError::Template(e.to_string()))?
        {
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

    async fn save(&self, name: &str, body: &str) -> Result<(), CoreError> {
        let dir = self.dir();
        // Create the prompts dir on first save so a fresh profile (or a moved
        // prompts path) does not fail the very first template creation.
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| CoreError::Template(e.to_string()))?;

        // Atomic write: stream into a unique sibling temp file, then rename over
        // the destination. A crash mid-write leaves the old template intact and
        // only an orphan `.tmp` behind — never a half-written `.md`.
        let dest = dir.join(format!("{name}.md"));
        let tmp = dir.join(format!(
            ".{name}.{}.tmp",
            std::process::id() as u64 * 1_000_000 + rand_suffix()
        ));
        tokio::fs::write(&tmp, body)
            .await
            .map_err(|e| CoreError::Template(e.to_string()))?;
        match tokio::fs::rename(&tmp, &dest).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                Err(CoreError::Template(e.to_string()))
            }
        }
    }

    async fn delete(&self, name: &str) -> Result<(), CoreError> {
        let path = self.path_for(name);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CoreError::NotFound(format!("template '{name}'")))
            }
            Err(e) => Err(CoreError::Template(e.to_string())),
        }
    }

    async fn rename(&self, old: &str, new: &str) -> Result<(), CoreError> {
        let from = self.path_for(old);
        let to = self.path_for(new);
        if !from.exists() {
            return Err(CoreError::NotFound(format!("template '{old}'")));
        }
        // Same-directory rename is atomic on every supported platform.
        tokio::fs::rename(&from, &to)
            .await
            .map_err(|e| CoreError::Template(e.to_string()))
    }
}

/// A small per-call nonce for the temp file name. Not cryptographic — only used
/// so two concurrent saves of the same template never pick the same temp path.
fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
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

    #[tokio::test]
    async fn save_then_load_roundtrip() {
        let tmp = setup();
        let loader = FileTemplateLoader::new(tmp.path());
        loader
            .save("Ретро", "Сделай ретро-протокол.")
            .await
            .unwrap();

        assert!(tmp.path().join("Ретро.md").exists());
        assert_eq!(
            loader.load("Ретро").await.unwrap().unwrap(),
            "Сделай ретро-протокол."
        );
    }

    #[tokio::test]
    async fn save_creates_missing_prompts_dir() {
        let tmp = setup();
        let nested = tmp.path().join("does/not/exist/yet");
        let loader = FileTemplateLoader::new(&nested);
        loader.save("X", "body").await.unwrap();
        assert!(nested.join("X.md").exists());
    }

    #[tokio::test]
    async fn delete_removes_file_and_reports_missing() {
        let tmp = setup();
        fs::write(tmp.path().join("X.md"), "body").unwrap();
        let loader = FileTemplateLoader::new(tmp.path());

        loader.delete("X").await.unwrap();
        assert!(!tmp.path().join("X.md").exists());

        let err = loader.delete("X").await.unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
    }

    #[tokio::test]
    async fn rename_moves_body_and_reports_missing_source() {
        let tmp = setup();
        fs::write(tmp.path().join("Old.md"), "keep me").unwrap();
        let loader = FileTemplateLoader::new(tmp.path());

        loader.rename("Old", "New").await.unwrap();
        assert!(!tmp.path().join("Old.md").exists());
        assert_eq!(loader.load("New").await.unwrap().unwrap(), "keep me");

        let err = loader.rename("Old", "Whatever").await.unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
    }
}
