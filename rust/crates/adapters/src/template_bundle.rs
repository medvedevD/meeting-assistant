use meeting_core::ports::TemplateBundle;

/// Language of the bundled templates a process seeds. It is fixed for one run:
/// the GUI passes it to the sidecar at launch, and the legacy CLI uses the
/// default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TemplateLanguage {
    #[default]
    Russian,
    English,
}

impl TemplateLanguage {
    /// Parses the code the GUI passes in `--language`.
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "ru" => Some(Self::Russian),
            "en" => Some(Self::English),
            _ => None,
        }
    }
}

/// The prompt templates shipped with the application, embedded in the binary at
/// compile time. Mirrors the `MIGRATIONS` const in [`crate::db`]: a new bundled
/// template is "drop a `.md` into `prompts/<language>/` + add one line here", and
/// it then reaches upgraded installs via
/// [`meeting_core::usecases::backfill_templates`].
///
/// Names must be unique across languages, because the tombstones in
/// `settings.removed_bundled_templates` are keyed by name alone.
///
/// Paths are relative to this source file: `crates/adapters/src/` → repo-root
/// `prompts/` is four levels up.
const BUNDLED: &[(TemplateLanguage, &str, &str)] = &[
    (
        TemplateLanguage::Russian,
        "Дейлик",
        include_str!("../../../../prompts/ru/Дейлик.md"),
    ),
    (
        TemplateLanguage::Russian,
        "1-на-1",
        include_str!("../../../../prompts/ru/1-на-1.md"),
    ),
    (
        TemplateLanguage::Russian,
        "Командная встреча",
        include_str!("../../../../prompts/ru/Командная встреча.md"),
    ),
    (
        TemplateLanguage::Russian,
        "Простой протокол",
        include_str!("../../../../prompts/ru/Простой протокол.md"),
    ),
    (
        TemplateLanguage::English,
        "Daily stand-up",
        include_str!("../../../../prompts/en/Daily stand-up.md"),
    ),
    (
        TemplateLanguage::English,
        "1-on-1",
        include_str!("../../../../prompts/en/1-on-1.md"),
    ),
    (
        TemplateLanguage::English,
        "Team meeting",
        include_str!("../../../../prompts/en/Team meeting.md"),
    ),
    (
        TemplateLanguage::English,
        "Simple minutes",
        include_str!("../../../../prompts/en/Simple minutes.md"),
    ),
];

/// True if `name` is a template shipped with the app, in any language. A deletion
/// made while the app runs in one language must still be remembered when it later
/// runs in the other.
pub fn is_bundled_template(name: &str) -> bool {
    BUNDLED.iter().any(|(_, bundled, _)| *bundled == name)
}

/// [`TemplateBundle`] backed by one language of the compile-time–embedded
/// [`BUNDLED`] set.
pub struct EmbeddedBundle {
    language: TemplateLanguage,
}

impl EmbeddedBundle {
    pub fn new(language: TemplateLanguage) -> Self {
        Self { language }
    }
}

impl TemplateBundle for EmbeddedBundle {
    fn entries(&self) -> Vec<(String, String)> {
        BUNDLED
            .iter()
            .filter(|(language, _, _)| *language == self.language)
            .map(|(_, name, body)| (name.to_string(), body.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(language: TemplateLanguage) -> Vec<String> {
        EmbeddedBundle::new(language)
            .entries()
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn embeds_the_known_templates_with_non_empty_bodies() {
        assert_eq!(
            names(TemplateLanguage::Russian),
            ["Дейлик", "1-на-1", "Командная встреча", "Простой протокол"]
        );
        assert_eq!(
            names(TemplateLanguage::English),
            ["Daily stand-up", "1-on-1", "Team meeting", "Simple minutes"]
        );
        assert!(BUNDLED.iter().all(|(_, _, body)| !body.trim().is_empty()));
    }

    #[test]
    fn english_templates_contain_no_cyrillic_and_ask_for_english() {
        for (name, body) in EmbeddedBundle::new(TemplateLanguage::English).entries() {
            assert!(
                !body.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)),
                "{name} contains Cyrillic"
            );
            assert!(
                body.contains("in English"),
                "{name} fixes no output language"
            );
            assert!(
                body.contains("{transcript}"),
                "{name} has no transcript slot"
            );
        }
    }

    #[test]
    fn names_are_unique_across_languages() {
        let mut all: Vec<&str> = BUNDLED.iter().map(|(_, name, _)| *name).collect();
        all.sort_unstable();
        let total = all.len();
        all.dedup();
        assert_eq!(all.len(), total, "tombstones are keyed by name alone");
    }

    #[test]
    fn language_parse_accepts_only_known_codes() {
        assert_eq!(
            TemplateLanguage::parse("ru"),
            Some(TemplateLanguage::Russian)
        );
        assert_eq!(
            TemplateLanguage::parse("en"),
            Some(TemplateLanguage::English)
        );
        assert_eq!(TemplateLanguage::parse("de"), None);
        assert_eq!(TemplateLanguage::default(), TemplateLanguage::Russian);
    }

    #[test]
    fn is_bundled_template_covers_every_language() {
        assert!(is_bundled_template("Дейлик"));
        assert!(is_bundled_template("Daily stand-up"));
        assert!(!is_bundled_template("My own template"));
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
        let bundle = EmbeddedBundle::new(TemplateLanguage::Russian);

        // First run seeds everything onto disk.
        let seeded = backfill_templates(&bundle, &loader, &[]).await.unwrap();
        assert_eq!(seeded.len(), bundle.entries().len());
        assert!(tmp.path().join("Дейлик.md").exists());

        // User edits a bundled template, then a deleted one is tombstoned.
        loader.save("Дейлик", "my custom daily").await.unwrap();
        let removed = vec!["Простой протокол".to_string()];
        std::fs::remove_file(tmp.path().join("Простой протокол.md")).unwrap();

        // Second run preserves the edit and respects the tombstone.
        let seeded = backfill_templates(&bundle, &loader, &removed)
            .await
            .unwrap();
        assert!(seeded.is_empty());
        assert_eq!(
            loader.load("Дейлик").await.unwrap().unwrap(),
            "my custom daily"
        );
        assert!(!tmp.path().join("Простой протокол.md").exists());
    }

    #[tokio::test]
    async fn fresh_english_install_gets_only_english_templates() {
        use crate::FileTemplateLoader;
        use meeting_core::ports::TemplateLoader;
        use meeting_core::usecases::backfill_templates;

        let tmp = tempfile::tempdir().unwrap();
        let loader = FileTemplateLoader::new(tmp.path());

        let seeded = backfill_templates(
            &EmbeddedBundle::new(TemplateLanguage::English),
            &loader,
            &[],
        )
        .await
        .unwrap();

        assert_eq!(seeded, names(TemplateLanguage::English));
        let mut on_disk = loader.list_names().await.unwrap();
        on_disk.sort_unstable();
        let mut expected = names(TemplateLanguage::English);
        expected.sort_unstable();
        assert_eq!(on_disk, expected);
    }

    // An upgrade must not disturb a Russian profile: same run language, every
    // bundled name already handled, so nothing new appears.
    #[tokio::test]
    async fn existing_russian_profile_is_unchanged_by_the_upgrade() {
        use crate::FileTemplateLoader;
        use meeting_core::ports::TemplateLoader;
        use meeting_core::usecases::backfill_templates;

        let tmp = tempfile::tempdir().unwrap();
        let loader = FileTemplateLoader::new(tmp.path());
        loader.save("Дейлик", "my custom daily").await.unwrap();
        loader
            .save("1-на-1", include_str!("../../../../prompts/ru/1-на-1.md"))
            .await
            .unwrap();
        loader.save("Мой шаблон", "own template").await.unwrap();
        // "Простой протокол" was deleted by the user; "Командная встреча" renamed.
        let removed = vec![
            "Простой протокол".to_string(),
            "Командная встреча".to_string(),
        ];

        let seeded = backfill_templates(
            &EmbeddedBundle::new(TemplateLanguage::Russian),
            &loader,
            &removed,
        )
        .await
        .unwrap();

        assert!(seeded.is_empty(), "no new templates, got {seeded:?}");
        assert_eq!(
            loader.load("Дейлик").await.unwrap().unwrap(),
            "my custom daily"
        );
        assert_eq!(
            loader.load("Мой шаблон").await.unwrap().unwrap(),
            "own template"
        );
        assert_eq!(loader.load("Простой протокол").await.unwrap(), None);
        assert_eq!(loader.load("Daily stand-up").await.unwrap(), None);
    }
}
