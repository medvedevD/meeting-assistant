use crate::{
    entities::Protocol,
    ports::{LlmProvider, TemplateLoader},
    CoreError,
};
use std::sync::Arc;

fn build_message(template: &str, transcript: &str, meeting_name: Option<&str>) -> String {
    let with_transcript = template.replace("{transcript}", transcript);
    match meeting_name {
        Some(name) => with_transcript.replace("{meeting_name}", name),
        None => with_transcript,
    }
}

/// Used when no template is configured. It fixes no output language, so the
/// minutes come out in the language of the meeting.
const DEFAULT_PROMPT: &str = "\
Write brief structured meeting minutes based on the following transcript, in the language of the transcript. \
Highlight the key topics, decisions, and action items.

Transcript:
{transcript}";

pub async fn generate_protocol(
    llm: Arc<dyn LlmProvider>,
    templates: Arc<dyn TemplateLoader>,
    transcript: &str,
    template_name: Option<&str>,
    meeting_name: Option<&str>,
) -> Result<Protocol, CoreError> {
    let message = match template_name {
        None => build_message(DEFAULT_PROMPT, transcript, meeting_name),
        Some(name) => match templates.load(name).await? {
            None => return Err(CoreError::Template(format!("template '{name}' not found"))),
            Some(tmpl) => build_message(&tmpl, transcript, meeting_name),
        },
    };

    let completion = llm.generate(&message, None).await?;

    Ok(Protocol {
        usage: completion.usage,
        ..Protocol::new(completion.text)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::TokenUsage;
    use crate::fakes::{FakeLlmProvider, FakeTemplateLoader};
    use crate::ports::LlmCompletion;

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
    async fn carries_the_token_usage_the_provider_reported() {
        let usage = TokenUsage {
            input_tokens: 1200,
            output_tokens: 340,
        };
        let llm = FakeLlmProvider::with_usage("# Протокол", usage);
        let templates = FakeTemplateLoader::empty();

        let result = generate_protocol(llm, templates, "текст", None, None)
            .await
            .unwrap();

        assert_eq!(result.usage, Some(usage));
    }

    #[tokio::test]
    async fn usage_is_none_when_the_provider_reports_none() {
        let llm = FakeLlmProvider::new("# Протокол");
        let templates = FakeTemplateLoader::empty();

        let result = generate_protocol(llm, templates, "текст", None, None)
            .await
            .unwrap();

        assert_eq!(result.usage, None);
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
            async fn generate(&self, _: &str, _: Option<&str>) -> Result<LlmCompletion, CoreError> {
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
    fn fallback_prompt_fixes_no_output_language() {
        assert!(
            !DEFAULT_PROMPT
                .chars()
                .any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)),
            "must not carry Russian"
        );
        let lower = DEFAULT_PROMPT.to_lowercase();
        for fixed in ["in english", "in russian"] {
            assert!(!lower.contains(fixed), "must not ask for {fixed:?}");
        }
        assert!(lower.contains("in the language of the transcript"));
    }

    #[test]
    fn build_message_substitutes_transcript() {
        let tmpl = "Intro.\n{transcript}\nOutro.";
        let result = build_message(tmpl, "transcript text", None);
        assert!(!result.contains("{transcript}"));
        assert!(result.contains("transcript text"));
        assert!(result.contains("Intro."));
        assert!(result.contains("Outro."));
    }

    #[test]
    fn build_message_substitutes_meeting_name() {
        let tmpl = "Встреча: {meeting_name}\n{transcript}";
        let result = build_message(tmpl, "text", Some("1-на-1"));
        assert!(result.contains("1-на-1"));
        assert!(!result.contains("{meeting_name}"));
    }

    #[test]
    fn build_message_only_transcript_placeholder_returns_transcript() {
        let tmpl = "{transcript}";
        let result = build_message(tmpl, "hello", None);
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn no_template_succeeds_with_default_prompt() {
        let llm = FakeLlmProvider::new("# Протокол\n\nОК.");
        let templates = FakeTemplateLoader::empty();

        let result = generate_protocol(llm, templates, "текст встречи", None, None)
            .await
            .unwrap();

        assert_eq!(result.markdown, "# Протокол\n\nОК.");
    }
}
