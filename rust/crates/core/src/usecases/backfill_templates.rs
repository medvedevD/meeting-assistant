use crate::ports::{TemplateBundle, TemplateLoader};
use crate::CoreError;

/// Seed every bundled template that is absent from `loader`, skipping names
/// listed in `removed`.
///
/// Never overwrites a template that already exists on disk, so a user's edits
/// to a bundled template — and any template they authored — survive across
/// upgrades. A bundled template the user deliberately deleted is listed in
/// `removed` and stays gone. Idempotent: a second run with the same inputs
/// writes nothing.
///
/// Returns the names that were newly written, in bundle order, for logging.
pub async fn backfill_templates(
    bundle: &dyn TemplateBundle,
    loader: &dyn TemplateLoader,
    removed: &[String],
) -> Result<Vec<String>, CoreError> {
    let mut seeded = Vec::new();
    for (name, body) in bundle.entries() {
        if removed.iter().any(|r| r == &name) {
            continue;
        }
        if loader.load(&name).await?.is_none() {
            loader.save(&name, &body).await?;
            seeded.push(name);
        }
    }
    Ok(seeded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeTemplateBundle, FakeTemplateLoader};
    use std::sync::Arc;

    fn bundle() -> FakeTemplateBundle {
        FakeTemplateBundle::new([("Дейлик", "daily body"), ("Ретро", "retro body")])
    }

    #[tokio::test]
    async fn seeds_all_into_empty_loader() {
        let loader: Arc<dyn TemplateLoader> = FakeTemplateLoader::empty();
        let seeded = backfill_templates(&bundle(), loader.as_ref(), &[])
            .await
            .unwrap();

        assert_eq!(seeded, vec!["Дейлик".to_string(), "Ретро".to_string()]);
        assert_eq!(loader.load("Дейлик").await.unwrap().unwrap(), "daily body");
        assert_eq!(loader.load("Ретро").await.unwrap().unwrap(), "retro body");
    }

    #[tokio::test]
    async fn never_overwrites_existing_body() {
        let loader: Arc<dyn TemplateLoader> = FakeTemplateLoader::new([("Дейлик", "my edits")]);
        let seeded = backfill_templates(&bundle(), loader.as_ref(), &[])
            .await
            .unwrap();

        assert_eq!(seeded, vec!["Ретро".to_string()]);
        assert_eq!(loader.load("Дейлик").await.unwrap().unwrap(), "my edits");
    }

    #[tokio::test]
    async fn skips_names_in_removed_list() {
        let loader: Arc<dyn TemplateLoader> = FakeTemplateLoader::empty();
        let removed = vec!["Дейлик".to_string()];
        let seeded = backfill_templates(&bundle(), loader.as_ref(), &removed)
            .await
            .unwrap();

        assert_eq!(seeded, vec!["Ретро".to_string()]);
        assert!(loader.load("Дейлик").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn second_run_is_a_noop() {
        let loader: Arc<dyn TemplateLoader> = FakeTemplateLoader::empty();
        backfill_templates(&bundle(), loader.as_ref(), &[])
            .await
            .unwrap();
        let seeded = backfill_templates(&bundle(), loader.as_ref(), &[])
            .await
            .unwrap();
        assert!(seeded.is_empty());
    }
}
