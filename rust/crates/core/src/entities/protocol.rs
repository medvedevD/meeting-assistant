use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protocol {
    pub markdown: String,
    pub structured: StructuredProtocol,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredProtocol {
    pub title: Option<String>,
    pub summary: Vec<String>,
    pub topics: Vec<ProtocolTopic>,
    pub decisions: Vec<String>,
    pub actions: Vec<ProtocolAction>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTopic {
    pub title: String,
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolAction {
    pub title: String,
    pub owner: Option<String>,
    pub due: Option<String>,
}

impl Protocol {
    pub fn new(markdown: impl Into<String>) -> Self {
        let markdown = markdown.into();
        let structured = StructuredProtocol::from_markdown(&markdown);
        Self {
            markdown,
            structured,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    None,
    Summary,
    Topics,
    Decisions,
    Actions,
    OpenQuestions,
}

impl StructuredProtocol {
    pub fn from_markdown(markdown: &str) -> Self {
        let mut out = StructuredProtocol::default();
        let mut section = Section::None;
        let mut current_topic: Option<ProtocolTopic> = None;

        for raw in markdown.lines() {
            let line = raw.trim();
            if line.is_empty() || is_rule(line) {
                continue;
            }

            if let Some((level, heading)) = heading_text(line) {
                flush_topic(&mut out, &mut current_topic);
                // Only a top-level `#` heading is a document title. Section
                // labels are `##` (e.g. "## Тип встречи"), and promoting one to
                // the title rendered it as a giant headline that duplicated the
                // meeting name already shown in the header chrome.
                if level == 1 && out.title.is_none() {
                    out.title = Some(heading.clone());
                }
                let next_section = classify_section(&heading);
                if section == Section::Topics && next_section == Section::None {
                    current_topic = Some(ProtocolTopic {
                        title: heading,
                        bullets: Vec::new(),
                    });
                    continue;
                }
                section = next_section;
                if section == Section::Topics && !is_generic_topic_heading(&heading) {
                    current_topic = Some(ProtocolTopic {
                        title: heading,
                        bullets: Vec::new(),
                    });
                }
                continue;
            }

            if is_table_separator(line) {
                continue;
            }

            if line.contains('|') {
                let row = table_cells(line);
                if section == Section::Actions && !looks_like_table_header(&row) {
                    if let Some(action) = action_from_cells(&row) {
                        out.actions.push(action);
                    }
                }
                continue;
            }

            let item = clean_line(line);
            if item.is_empty() || is_empty_placeholder(&item) {
                continue;
            }

            match section {
                Section::Summary => out.summary.push(item),
                Section::Decisions => out.decisions.push(item),
                Section::Actions => out.actions.push(action_from_text(&item)),
                Section::OpenQuestions => out.open_questions.push(item),
                Section::Topics => {
                    if current_topic.is_none() {
                        current_topic = Some(ProtocolTopic {
                            title: item,
                            bullets: Vec::new(),
                        });
                    } else if let Some(topic) = current_topic.as_mut() {
                        topic.bullets.push(item);
                    }
                }
                Section::None => {
                    if out.summary.is_empty() && !is_probably_metadata(&item) {
                        out.summary.push(item);
                    }
                }
            }
        }

        flush_topic(&mut out, &mut current_topic);
        out
    }
}

fn flush_topic(out: &mut StructuredProtocol, current: &mut Option<ProtocolTopic>) {
    if let Some(topic) = current.take() {
        if !topic.title.is_empty() || !topic.bullets.is_empty() {
            out.topics.push(topic);
        }
    }
}

/// Returns the heading level (number of leading `#`) and its cleaned text.
fn heading_text(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 || !trimmed.chars().nth(hashes).is_some_and(char::is_whitespace) {
        return None;
    }
    Some((
        hashes,
        clean_inline(trimmed[hashes..].trim().trim_end_matches('#').trim()),
    ))
}

fn classify_section(title: &str) -> Section {
    let lower = title.to_lowercase();
    if lower.contains("решен") || lower.contains("договор") || lower.contains("decision")
    {
        Section::Decisions
    } else if lower.contains("action")
        || lower.contains("действ")
        || lower.contains("задач")
        || lower.contains("следующ")
    {
        Section::Actions
    } else if lower.contains("вопрос") || lower.contains("question") {
        Section::OpenQuestions
    } else if lower.contains("тем") || lower.contains("обсужд") || lower.contains("topic")
    {
        Section::Topics
    } else if lower.contains("крат")
        || lower.contains("резюм")
        || lower.contains("итог")
        || lower.contains("summary")
    {
        Section::Summary
    } else {
        Section::None
    }
}

fn is_generic_topic_heading(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.contains("тем") || lower.contains("обсужд") || lower.contains("topic")
}

fn clean_line(line: &str) -> String {
    let without_quote = line.trim_start_matches('>').trim();
    let without_marker = if without_quote.starts_with("- ") || without_quote.starts_with("* ") {
        &without_quote[2..]
    } else if let Some(dot) = without_quote.find(". ") {
        if without_quote[..dot].chars().all(|c| c.is_ascii_digit()) {
            &without_quote[dot + 2..]
        } else {
            without_quote
        }
    } else {
        without_quote
    };
    clean_inline(without_marker)
}

fn clean_inline(text: &str) -> String {
    let mut out = text.to_string();
    while let Some(open) = out.find('[') {
        let Some(close_rel) = out[open..].find("](") else {
            break;
        };
        let close = open + close_rel;
        let Some(end_rel) = out[close + 2..].find(')') else {
            break;
        };
        let end = close + 2 + end_rel;
        let label = out[open + 1..close].to_string();
        out.replace_range(open..=end, &label);
    }
    out = out.replace("**", "").replace("__", "");
    out = out.replace(['*', '_', '`', '[', ']'], "");
    out.trim().to_string()
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    matches!(compact.as_str(), "---" | "***" | "___") || compact.chars().all(|c| c == '-')
}

fn is_table_separator(line: &str) -> bool {
    let cells = table_cells(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| c.trim_matches(':').chars().all(|ch| ch == '-') && c.contains('-'))
}

fn table_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = line
        .trim()
        .trim_matches('|')
        .split('|')
        .map(clean_inline)
        .filter(|c| !c.is_empty())
        .collect();
    if cells.len() == 1 && cells[0].is_empty() {
        cells.clear();
    }
    cells
}

fn looks_like_table_header(cells: &[String]) -> bool {
    cells.iter().any(|cell| {
        let lower = cell.to_lowercase();
        lower.contains("owner")
            || lower.contains("ответ")
            || lower.contains("action")
            || lower.contains("действ")
            || lower.contains("срок")
            || lower.contains("due")
    })
}

fn action_from_cells(cells: &[String]) -> Option<ProtocolAction> {
    if cells.is_empty() {
        return None;
    }
    let owner = cells.first().cloned().filter(|s| !s.is_empty());
    let title = cells
        .get(1)
        .cloned()
        .or_else(|| cells.first().cloned())
        .unwrap_or_default();
    let due = cells
        .get(2)
        .cloned()
        .filter(|s| !s.is_empty() && !is_empty_placeholder(s));
    if title.is_empty() || is_empty_placeholder(&title) {
        None
    } else {
        Some(ProtocolAction { title, owner, due })
    }
}

fn action_from_text(text: &str) -> ProtocolAction {
    let separators = [" — ", " - ", ": "];
    for sep in separators {
        if let Some((owner, title)) = text.split_once(sep) {
            if owner.chars().count() <= 32 && !title.trim().is_empty() {
                return ProtocolAction {
                    title: title.trim().to_string(),
                    owner: Some(owner.trim().to_string()),
                    due: None,
                };
            }
        }
    }
    ProtocolAction {
        title: text.to_string(),
        owner: None,
        due: None,
    }
}

fn is_probably_metadata(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with("дата:") || lower.starts_with("участники:") || lower.starts_with("meeting:")
}

fn is_empty_placeholder(text: &str) -> bool {
    let compact = text
        .trim()
        .trim_matches('.')
        .trim_matches('—')
        .trim_matches('-')
        .trim()
        .to_lowercase();
    matches!(
        compact.as_str(),
        "" | "нет"
            | "нет."
            | "не применимо"
            | "n/a"
            | "na"
            | "none"
            | "отсутствует"
            | "отсутствуют"
            | "не обсуждалось"
            | "не было"
            | "не выявлено"
            | "нет данных"
            | "не указано"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_common_protocol_sections_from_markdown() {
        let md = "\
# Планирование спринта

## Краткое резюме
- Команда согласовала цель релиза.

## Темы
### Риски поставки
- Нужна проверка интеграции.
- Демо готово частично.

## Решения
- Выпустить beta в пятницу.

## Дальнейшие действия
Ответственный | Действие | Срок
--- | --- | ---
Дима | Подготовить сборку | пятница

## Открытые вопросы
- Кто проведет демо?";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(structured.title.as_deref(), Some("Планирование спринта"));
        assert_eq!(structured.summary, vec!["Команда согласовала цель релиза."]);
        assert_eq!(structured.topics.len(), 1);
        assert_eq!(structured.topics[0].title, "Риски поставки");
        assert_eq!(structured.topics[0].bullets.len(), 2);
        assert_eq!(structured.decisions, vec!["Выпустить beta в пятницу."]);
        assert_eq!(
            structured.actions,
            vec![ProtocolAction {
                title: "Подготовить сборку".into(),
                owner: Some("Дима".into()),
                due: Some("пятница".into()),
            }]
        );
        assert_eq!(structured.open_questions, vec!["Кто проведет демо?"]);
    }

    #[test]
    fn protocol_new_keeps_markdown_and_adds_structure() {
        let protocol = Protocol::new("# Протокол\n\n## Решения\n- Делать.");

        assert_eq!(protocol.markdown, "# Протокол\n\n## Решения\n- Делать.");
        assert_eq!(protocol.structured.title.as_deref(), Some("Протокол"));
        assert_eq!(protocol.structured.decisions, vec!["Делать."]);
    }

    #[test]
    fn skips_empty_placeholder_sections_and_tables() {
        let md = "\
# Протокол

## Решения
Нет

## Дальнейшие действия
| Задача | Ответственный | Срок |
|--------|---------------|------|

## Открытые вопросы
- Не обсуждалось";

        let structured = StructuredProtocol::from_markdown(md);

        assert!(structured.decisions.is_empty());
        assert!(structured.actions.is_empty());
        assert!(structured.open_questions.is_empty());
    }

    #[test]
    fn section_label_heading_is_not_promoted_to_title() {
        // The "Простой протокол" template leads with "## Тип встречи" — a field
        // label, not a document title. It must not become a giant headline.
        let md = "\
## Тип встречи
Неформальный разговор / дискуссия

## Краткое резюме
Спорили о жанре.";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(structured.title, None);
        assert_eq!(
            structured.summary,
            vec!["Неформальный разговор / дискуссия", "Спорили о жанре."]
        );
    }

    #[test]
    fn first_level_one_heading_still_becomes_title() {
        let structured = StructuredProtocol::from_markdown("# Планёрка\n\n## Решения\n- Го.");

        assert_eq!(structured.title.as_deref(), Some("Планёрка"));
    }
}
