use meeting_core::ports::TemplateBundle;

/// The prompt templates shipped with the application, embedded in the binary at
/// compile time. Mirrors the `MIGRATIONS` const in [`crate::db`]: a new bundled
/// template is "drop a `.md` into `prompts/` + add one line here", and it then
/// reaches upgraded installs via [`meeting_core::usecases::backfill_templates`].
///
/// Paths are relative to this source file: `crates/adapters/src/` → repo-root
/// `prompts/` is four levels up.
const BUNDLED: &[(&str, &str)] = &[
    ("Дейлик", include_str!("../../../../prompts/Дейлик.md")),
    ("1-на-1", include_str!("../../../../prompts/1-на-1.md")),
    (
        "Командная встреча",
        include_str!("../../../../prompts/Командная встреча.md"),
    ),
    (
        "Простой протокол",
        include_str!("../../../../prompts/Простой протокол.md"),
    ),
];

/// [`TemplateBundle`] backed by the compile-time–embedded [`BUNDLED`] set.
pub struct EmbeddedBundle;

impl TemplateBundle for EmbeddedBundle {
    fn entries(&self) -> Vec<(String, String)> {
        BUNDLED
            .iter()
            .map(|(name, body)| (name.to_string(), body.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_known_templates_with_non_empty_bodies() {
        let entries = EmbeddedBundle.entries();
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();

        assert!(names.contains(&"Дейлик"));
        assert!(names.contains(&"1-на-1"));
        assert!(names.contains(&"Командная встреча"));
        assert!(names.contains(&"Простой протокол"));
        assert!(entries.iter().all(|(_, body)| !body.trim().is_empty()));
    }

    // End-to-end seed through the real file-backed loader: the embedded bundle
    // lands on disk, a user edit is preserved on re-run, and a tombstoned name
    // is not resurrected.
    #[tokio::test]
    async fn seeds_through_the_real_file_loader() {
        use crate::FileTemplateLoader;
        use meeting_core::ports::TemplateLoader;
        use meeting_core::usecases::backfill_templates;

        let tmp = tempfile::tempdir().unwrap();
        let loader = FileTemplateLoader::new(tmp.path());

        // First run seeds everything onto disk.
        let seeded = backfill_templates(&EmbeddedBundle, &loader, &[])
            .await
            .unwrap();
        assert_eq!(seeded.len(), EmbeddedBundle.entries().len());
        assert!(tmp.path().join("Дейлик.md").exists());

        // User edits a bundled template, then a deleted one is tombstoned.
        loader.save("Дейлик", "my custom daily").await.unwrap();
        let removed = vec!["Простой протокол".to_string()];
        std::fs::remove_file(tmp.path().join("Простой протокол.md")).unwrap();

        // Second run preserves the edit and respects the tombstone.
        let seeded = backfill_templates(&EmbeddedBundle, &loader, &removed)
            .await
            .unwrap();
        assert!(seeded.is_empty());
        assert_eq!(
            loader.load("Дейлик").await.unwrap().unwrap(),
            "my custom daily"
        );
        assert!(!tmp.path().join("Простой протокол.md").exists());
    }
}
