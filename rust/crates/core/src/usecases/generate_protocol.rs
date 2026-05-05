use std::sync::Arc;
use crate::{CoreError, entities::Protocol, ports::{LlmProvider, TemplateLoader}};

/// Build instructions from a template: remove the `{transcript}` placeholder
/// (transcript is sent separately for caching) and substitute `{meeting_name}`.
fn build_instructions(template: &str, meeting_name: Option<&str>) -> String {
    let without_transcript = template.replace("{transcript}", "");
    match meeting_name {
        Some(name) => without_transcript.replace("{meeting_name}", name),
        None => without_transcript,
    }
}

pub async fn generate_protocol(
    llm: Arc<dyn LlmProvider>,
    templates: Arc<dyn TemplateLoader>,
    transcript: &str,
    template_name: Option<&str>,
    meeting_name: Option<&str>,
) -> Result<Protocol, CoreError> {
    let instructions = match template_name {
        None => None,
        Some(name) => match templates.load(name).await? {
            None => return Err(CoreError::Template(format!("template '{name}' not found"))),
            Some(tmpl) => {
                let built = build_instructions(&tmpl, meeting_name);
                if built.trim().is_empty() { None } else { Some(built) }
            }
        },
    };

    let markdown = llm
        .generate(transcript, instructions.as_deref())
        .await?;

    Ok(Protocol::new(markdown))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeLlmProvider, FakeTemplateLoader};

    #[tokio::test]
    async fn returns_protocol_without_template() {
        let llm = FakeLlmProvider::new("# Протокол\n\nВсё хорошо.");
        let templates = FakeTemplateLoader::empty();

        let result = generate_protocol(llm, templates, "текст встречи", None, None)
            .await
            .unwrap();

        assert_eq!(result.markdown, "# Протокол\n\nВсё хорошо.");
    }

    #[tokio::test]
    async fn passes_formatted_instructions_from_template() {
        let tmpl = "Сделай протокол для {meeting_name}.\n{transcript}\nКратко.";
        let llm = FakeLlmProvider::new("# Дейлик");
        let templates = FakeTemplateLoader::new([("Дейлик", tmpl)]);

        let result = generate_protocol(
            llm,
            templates,
            "transcript text",
            Some("Дейлик"),
            Some("Дейлик с командой"),
        )
        .await
        .unwrap();

        assert_eq!(result.markdown, "# Дейлик");
    }

    #[tokio::test]
    async fn error_on_missing_template() {
        let llm = FakeLlmProvider::new("irrelevant");
        let templates = FakeTemplateLoader::empty();

        let err = generate_protocol(llm, templates, "transcript", Some("НеСуществующий"), None)
            .await
            .unwrap_err();

        assert!(matches!(err, CoreError::Template(_)));
    }

    #[tokio::test]
    async fn llm_error_propagates() {
        use async_trait::async_trait;

        struct FailingLlm;
        #[async_trait]
        impl LlmProvider for FailingLlm {
            async fn generate(&self, _: &str, _: Option<&str>) -> Result<String, CoreError> {
                Err(CoreError::Llm("service unavailable".into()))
            }
        }

        let llm = Arc::new(FailingLlm);
        let templates = FakeTemplateLoader::empty();

        let err = generate_protocol(llm, templates, "transcript", None, None)
            .await
            .unwrap_err();

        assert!(matches!(err, CoreError::Llm(_)));
    }

    #[test]
    fn build_instructions_removes_transcript_placeholder() {
        let tmpl = "Intro.\n{transcript}\nOutro.";
        let result = build_instructions(tmpl, None);
        assert!(!result.contains("{transcript}"));
        assert!(result.contains("Intro."));
        assert!(result.contains("Outro."));
    }

    #[test]
    fn build_instructions_substitutes_meeting_name() {
        let tmpl = "Встреча: {meeting_name}\n{transcript}";
        let result = build_instructions(tmpl, Some("1-на-1"));
        assert!(result.contains("1-на-1"));
        assert!(!result.contains("{meeting_name}"));
    }

    #[test]
    fn build_instructions_empty_after_removing_placeholder_only() {
        let tmpl = "{transcript}";
        let result = build_instructions(tmpl, None);
        assert!(result.trim().is_empty());
    }
}
