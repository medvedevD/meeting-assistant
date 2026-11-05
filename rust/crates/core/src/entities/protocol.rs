use super::TokenUsage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protocol {
    pub markdown: String,
    pub structured: StructuredProtocol,
    /// Tokens the LLM spent on this protocol; `None` when the provider did not
    /// report usage.
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredProtocol {
    pub title: Option<String>,
    pub summary: Vec<String>,
    pub topics: Vec<ProtocolTopic>,
    pub decisions: Vec<String>,
    pub actions: Vec<ProtocolAction>,
    pub open_questions: Vec<String>,
    /// Sections that fit none of the fields above, in document order.
    #[serde(default)]
    pub other_sections: Vec<ProtocolSection>,
}

/// A section outside the fixed set, such as "Статус по участникам".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSection {
    pub title: String,
    /// The section body as written: paragraphs, lists, tables, and `###`
    /// subsections. The GUI's Markdown renderer shows it directly.
    pub markdown: String,
    /// The `StructuredProtocol` field of the next fixed section in the document,
    /// so the GUI can keep this section in place. `None` when none follows.
    pub before: Option<String>,
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
            usage: None,
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

impl Section {
    /// The `StructuredProtocol` field this section fills.
    fn field_name(self) -> Option<&'static str> {
        match self {
            Section::None => None,
            Section::Summary => Some("summary"),
            Section::Topics => Some("topics"),
            Section::Decisions => Some("decisions"),
            Section::Actions => Some("actions"),
            Section::OpenQuestions => Some("open_questions"),
        }
    }
}

impl StructuredProtocol {
    pub fn from_markdown(markdown: &str) -> Self {
        let mut out = StructuredProtocol::default();
        let mut section = Section::None;
        // Level of the heading that opened `section`.
        let mut section_level = 0;
        let mut current_topic: Option<ProtocolTopic> = None;
        let mut current_other: Option<ProtocolSection> = None;
        let mut action_columns: Option<ActionColumns> = None;

        let mut lines = markdown.lines().peekable();
        while let Some(raw) = lines.next() {
            let line = raw.trim();
            if !line.contains('|') {
                action_columns = None;
            }
            if line.is_empty() || is_rule(line) {
                if let Some(other) = current_other.as_mut() {
                    other.markdown.push('\n');
                }
                continue;
            }

            if let Some((level, heading)) = heading_text(line) {
                // A deeper heading is a subsection, whatever it says: "### Артем"
                // under "Статус по участникам" is a person, not a topic ("тем").
                if let Some(other) = current_other.as_mut().filter(|_| level > section_level) {
                    push_body_line(other, raw);
                    continue;
                }
                flush_topic(&mut out, &mut current_topic);
                flush_other_section(&mut out, &mut current_other);
                // Only a top-level `#` heading is a document title. Section
                // labels are `##` (e.g. "## Тип встречи"), and promoting one to
                // the title rendered it as a giant headline that duplicated the
                // meeting name already shown in the header chrome.
                let is_title = level == 1 && out.title.is_none();
                if is_title {
                    out.title = Some(heading.clone());
                }
                let next_section = classify_section(&heading);
                if section == Section::Topics
                    && next_section == Section::None
                    && level > section_level
                {
                    current_topic = Some(ProtocolTopic {
                        title: heading,
                        bullets: Vec::new(),
                    });
                    continue;
                }
                section = next_section;
                section_level = level;
                if let Some(field) = section.field_name() {
                    for other in &mut out.other_sections {
                        other.before.get_or_insert_with(|| field.to_string());
                    }
                }
                if section == Section::Topics && !is_generic_topic_heading(&heading) {
                    current_topic = Some(ProtocolTopic {
                        title: heading,
                        bullets: Vec::new(),
                    });
                } else if section == Section::None && !is_title {
                    current_other = Some(ProtocolSection {
                        title: heading,
                        markdown: String::new(),
                        before: None,
                    });
                }
                continue;
            }

            if let Some(other) = current_other.as_mut() {
                if !is_empty_placeholder(&clean_line(line)) {
                    push_body_line(other, raw);
                }
                continue;
            }

            if is_table_separator(line) {
                continue;
            }

            if line.contains('|') {
                if section == Section::Actions {
                    let is_header = lines.peek().is_some_and(|next| is_table_separator(next));
                    if is_header {
                        action_columns = ActionColumns::from_header(&table_row(line));
                    }
                    let action = match action_columns {
                        Some(_) if is_header => None,
                        Some(columns) => columns.action(&table_row(line)),
                        None => {
                            let row = table_cells(line);
                            if looks_like_table_header(&row) {
                                None
                            } else {
                                action_from_cells(&row)
                            }
                        }
                    };
                    out.actions.extend(action);
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
                // Text before the first section: anything under a heading went
                // to `current_other` above.
                Section::None => {
                    if !is_probably_metadata(&item) {
                        out.summary.push(item);
                    }
                }
            }
        }

        flush_topic(&mut out, &mut current_topic);
        flush_other_section(&mut out, &mut current_other);
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

fn push_body_line(section: &mut ProtocolSection, line: &str) {
    section.markdown.push_str(line);
    section.markdown.push('\n');
}

fn flush_other_section(out: &mut StructuredProtocol, current: &mut Option<ProtocolSection>) {
    if let Some(mut section) = current.take() {
        section.markdown = section.markdown.trim().to_string();
        if !section.markdown.is_empty() {
            out.other_sections.push(section);
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
    if lower.contains("решен")
        || lower.contains("договор")
        || lower.contains("decision")
        || lower.contains("agreement")
    {
        Section::Decisions
    } else if lower.contains("action")
        || lower.contains("действ")
        || lower.contains("задач")
        || lower.contains("task")
        || lower.contains("следующ")
        || lower.contains("next")
    {
        Section::Actions
    } else if lower.contains("вопрос") || lower.contains("question") {
        Section::OpenQuestions
    } else if is_generic_topic_heading(title) {
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
    lower.contains("тем")
        || lower.contains("обсужд")
        || lower.contains("topic")
        || lower.contains("discuss")
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

/// Every cell of a table row, empty ones included, so positions match the header.
fn table_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(clean_inline).collect()
}

fn table_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = table_row(line)
        .into_iter()
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

/// Column positions of an action table, read from its header row.
#[derive(Clone, Copy)]
struct ActionColumns {
    title: usize,
    owner: Option<usize>,
    due: Option<usize>,
}

impl ActionColumns {
    /// `None` when the header names no task column. A column takes one role
    /// only, and owner and due choose first: "Срок задачи" is a due date.
    fn from_header(header: &[String]) -> Option<Self> {
        let column = |stems: &[&str], taken: &[Option<usize>]| {
            header.iter().enumerate().position(|(index, cell)| {
                let lower = cell.to_lowercase();
                !taken.contains(&Some(index)) && stems.iter().any(|stem| lower.contains(stem))
            })
        };
        let owner = column(
            &[
                "ответ",
                "исполн",
                "кто",
                "owner",
                "assignee",
                "responsible",
                "who",
            ],
            &[],
        );
        let due = column(
            &[
                "срок",
                "дедлайн",
                "когда",
                "дата",
                "due",
                "deadline",
                "date",
                "when",
            ],
            &[owner],
        );
        let title = column(
            &[
                "задач",
                "действ",
                "поручен",
                "описан",
                "что",
                "task",
                "action",
                "item",
                "descr",
                "what",
            ],
            &[owner, due],
        )?;
        Some(Self { title, owner, due })
    }

    fn action(self, row: &[String]) -> Option<ProtocolAction> {
        let cell = |index: Option<usize>| {
            index
                .and_then(|i| row.get(i))
                .filter(|c| !is_empty_placeholder(c))
                .cloned()
        };
        Some(ProtocolAction {
            title: cell(Some(self.title))?,
            owner: cell(self.owner),
            due: cell(self.due),
        })
    }
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
    lower.starts_with("дата:")
        || lower.starts_with("участники:")
        || lower.starts_with("date:")
        || lower.starts_with("participants:")
        || lower.starts_with("meeting:")
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
            | "no"
            | "not applicable"
            | "not discussed"
            | "not mentioned"
            | "not specified"
            | "no data"
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
        assert_eq!(structured.summary, vec!["Спорили о жанре."]);
        assert_eq!(
            structured.other_sections,
            vec![ProtocolSection {
                title: "Тип встречи".into(),
                markdown: "Неформальный разговор / дискуссия".into(),
                before: Some("summary".into()),
            }]
        );
    }

    /// The text of every `##` section of the bundled templates has to reach the
    /// model, in both languages, so a heading of a new template cannot lose it.
    #[test]
    fn no_template_section_loses_its_text() {
        const TEMPLATES: [&str; 8] = [
            include_str!("../../../../../prompts/en/1-on-1.md"),
            include_str!("../../../../../prompts/en/Daily stand-up.md"),
            include_str!("../../../../../prompts/en/Team meeting.md"),
            include_str!("../../../../../prompts/en/Simple minutes.md"),
            include_str!("../../../../../prompts/ru/1-на-1.md"),
            include_str!("../../../../../prompts/ru/Дейлик.md"),
            include_str!("../../../../../prompts/ru/Командная встреча.md"),
            include_str!("../../../../../prompts/ru/Простой протокол.md"),
        ];

        for template in TEMPLATES {
            let headings: Vec<&str> = template
                .lines()
                .filter_map(|l| l.strip_prefix("## "))
                .collect();
            assert!(headings.len() >= 4, "too few headings: {headings:?}");
            let md: String = headings
                .iter()
                .enumerate()
                .map(|(i, heading)| format!("## {heading}\nmarker{i};\n\n"))
                .collect();

            let dump = format!("{:?}", StructuredProtocol::from_markdown(&md));

            for (i, heading) in headings.iter().enumerate() {
                assert!(dump.contains(&format!("marker{i};")), "{heading}: {dump}");
            }
        }
    }

    #[test]
    fn keeps_the_stand_up_status_with_its_subsections() {
        let md = "\
## Статус по участникам

### Артем
- **Сделано:** закрыл тикет
- **Блокеры:** нет доступа

### Мария
- **В работе / план:** релиз

## Общие блокеры и риски
Стенд падает.

## Решения и договорённости
- Выпускаем в пятницу.";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(
            structured.other_sections,
            vec![
                ProtocolSection {
                    title: "Статус по участникам".into(),
                    markdown: "### Артем\n\
                               - **Сделано:** закрыл тикет\n\
                               - **Блокеры:** нет доступа\n\n\
                               ### Мария\n\
                               - **В работе / план:** релиз"
                        .into(),
                    before: Some("decisions".into()),
                },
                ProtocolSection {
                    title: "Общие блокеры и риски".into(),
                    markdown: "Стенд падает.".into(),
                    before: Some("decisions".into()),
                },
            ]
        );
        assert!(structured.summary.is_empty());
        assert!(structured.topics.is_empty(), "Артем is not a topic");
        assert_eq!(structured.decisions, vec!["Выпускаем в пятницу."]);
    }

    #[test]
    fn other_sections_keep_their_place_between_the_fixed_ones() {
        let md = "\
## Meeting goal
Agree on the release date.

## Participants
- Dana
- Lee

## Summary
The team reviewed the plan.

## Topics discussed
### Delivery
The build is late.

## Key decisions
- Ship on Friday.

## Risks and issues
| Risk | Impact |
|------|--------|
| Unstable stage | High |

## Action items
- Dana: prepare the build

## Open questions
- Who runs the demo?

## Appendix
Notes.";

        let structured = StructuredProtocol::from_markdown(md);

        let places: Vec<(&str, Option<&str>)> = structured
            .other_sections
            .iter()
            .map(|s| (s.title.as_str(), s.before.as_deref()))
            .collect();
        assert_eq!(
            places,
            vec![
                ("Meeting goal", Some("summary")),
                ("Participants", Some("summary")),
                ("Risks and issues", Some("actions")),
                ("Appendix", None),
            ]
        );
        assert_eq!(structured.other_sections[1].markdown, "- Dana\n- Lee");
        assert_eq!(
            structured.other_sections[2].markdown,
            "| Risk | Impact |\n|------|--------|\n| Unstable stage | High |"
        );
    }

    #[test]
    fn a_section_after_topics_is_not_another_topic() {
        let md = "\
## Обсуждённые темы
### Поставка
- Сборка опаздывает.

## Риски и проблемы
- Стенд падает.";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(structured.topics.len(), 1);
        assert_eq!(structured.topics[0].title, "Поставка");
        assert_eq!(structured.other_sections.len(), 1);
        assert_eq!(structured.other_sections[0].title, "Риски и проблемы");
        assert_eq!(structured.other_sections[0].markdown, "- Стенд падает.");
    }

    #[test]
    fn skips_other_sections_with_nothing_to_show() {
        let md = "\
## Участники
Не указано

## Обратная связь

## Feedback
- Not discussed

## Решения
- Го.";

        let structured = StructuredProtocol::from_markdown(md);

        assert!(structured.other_sections.is_empty());
        assert_eq!(structured.decisions, vec!["Го."]);
    }

    #[test]
    fn text_before_the_first_section_goes_to_the_summary() {
        let md = "\
# Планёрка
Дата: 5 ноября
Первая мысль.

Вторая мысль.

## Решения
- Го.";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(structured.title.as_deref(), Some("Планёрка"));
        assert_eq!(structured.summary, vec!["Первая мысль.", "Вторая мысль."]);
        assert!(structured.other_sections.is_empty());
    }

    fn action(title: &str, owner: Option<&str>, due: Option<&str>) -> ProtocolAction {
        ProtocolAction {
            title: title.into(),
            owner: owner.map(Into::into),
            due: due.map(Into::into),
        }
    }

    #[test]
    fn reads_action_columns_from_the_table_header() {
        for (header, row) in [
            (
                "| Задача | Ответственный | Срок |",
                "| Подготовить сборку | Дима | пятница |",
            ),
            (
                "| Task | Owner | Due |",
                "| Подготовить сборку | Дима | пятница |",
            ),
            (
                "| Owner | Task | Due |",
                "| Дима | Подготовить сборку | пятница |",
            ),
            (
                "| Ответственный | Действие | Срок |",
                "| Дима | Подготовить сборку | пятница |",
            ),
            (
                "| Срок | Исполнитель | Задача |",
                "| пятница | Дима | Подготовить сборку |",
            ),
            (
                "| # | Задача | Кто | Срок |",
                "| 1 | Подготовить сборку | Дима | пятница |",
            ),
        ] {
            let md = format!("## Action items\n{header}\n|---|---|---|\n{row}");

            let structured = StructuredProtocol::from_markdown(&md);

            assert_eq!(
                structured.actions,
                vec![action("Подготовить сборку", Some("Дима"), Some("пятница"))],
                "{header}"
            );
        }
    }

    #[test]
    fn empty_cells_do_not_shift_action_columns() {
        let md = "\
## Action items
| Задача | Ответственный | Срок |
|--------|--------------|------|
| Без срока | Дима | |
| Без ответственного |  | пятница |
| Прочерки | — | — |";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(
            structured.actions,
            vec![
                action("Без срока", Some("Дима"), None),
                action("Без ответственного", None, Some("пятница")),
                action("Прочерки", None, None),
            ]
        );
    }

    #[test]
    fn action_rows_may_mention_header_words() {
        let md = "\
## Action items
| Задача | Ответственный | Срок |
|--------|--------------|------|
| Подготовить ответ и уточнить срок | Дима | пятница |";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(
            structured.actions,
            vec![action(
                "Подготовить ответ и уточнить срок",
                Some("Дима"),
                Some("пятница")
            )]
        );
    }

    #[test]
    fn action_columns_apply_to_their_own_table_only() {
        let md = "\
## Action items
| Task | Owner |
|------|-------|
| Prepare the build | Dana |

| Owner | Task |
|-------|------|
| Lee | Run the demo |

Plain text ends the table
| Kim | Write the notes |";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(
            structured.actions,
            vec![
                action("Prepare the build", Some("Dana"), None),
                action("Run the demo", Some("Lee"), None),
                action("Plain text ends the table", None, None),
                action("Write the notes", Some("Kim"), None),
            ]
        );
    }

    #[test]
    fn action_columns_stay_positional_without_a_recognized_header() {
        let positional = action("Подготовить сборку", Some("Дима"), Some("пятница"));
        for md in [
            // No header at all.
            "## Action items\n| Дима | Подготовить сборку | пятница |",
            // A header row with no separator under it is skipped, as before.
            "## Action items\n| Задача | Ответственный | Срок |\n| Дима | Подготовить сборку | пятница |",
            // A separator under a row that names no task column.
            "## Action items\n| Ответственный | Срок |\n|---|---|\n| Дима | Подготовить сборку | пятница |",
        ] {
            let structured = StructuredProtocol::from_markdown(md);

            assert_eq!(structured.actions, vec![positional.clone()], "{md}");
        }
    }

    /// Every `##` heading of the bundled English templates, so a new heading
    /// cannot ship without a decision on where it belongs. Headings classified
    /// as `None` match their Russian counterparts, which are `None` too.
    #[test]
    fn classifies_every_heading_of_the_english_templates() {
        const TEMPLATES: [&str; 4] = [
            include_str!("../../../../../prompts/en/1-on-1.md"),
            include_str!("../../../../../prompts/en/Daily stand-up.md"),
            include_str!("../../../../../prompts/en/Team meeting.md"),
            include_str!("../../../../../prompts/en/Simple minutes.md"),
        ];
        let expected = |heading: &str| match heading {
            "Summary" => Section::Summary,
            "Topics discussed" | "Key topics" => Section::Topics,
            "Decisions and agreements" | "Key decisions" | "Decisions" => Section::Decisions,
            "Action items" | "Tasks / next steps" | "Task and project status" | "Next meeting" => {
                Section::Actions
            }
            "Open questions" => Section::OpenQuestions,
            "Problems and blockers"
            | "Feedback"
            | "Status by participant"
            | "Shared blockers and risks"
            | "Meeting goal"
            | "Participants"
            | "Risks and issues"
            | "Meeting type"
            | "Important conclusions / insights" => Section::None,
            other => panic!("no expected section for template heading {other:?}"),
        };

        let mut seen = 0;
        for template in TEMPLATES {
            for heading in template.lines().filter_map(|l| l.strip_prefix("## ")) {
                assert_eq!(classify_section(heading), expected(heading), "{heading}");
                seen += 1;
            }
        }
        assert!(seen >= 20, "expected the template headings, saw {seen}");
    }

    #[test]
    fn russian_headings_keep_their_sections() {
        for (heading, section) in [
            ("Краткое резюме", Section::Summary),
            ("Статус задач и проектов", Section::Actions),
            ("Проблемы и блокеры", Section::None),
            ("Решения и договорённости", Section::Decisions),
            ("Action items", Section::Actions),
            ("Следующая встреча", Section::Actions),
            ("Обсуждённые темы", Section::Topics),
            ("Открытые вопросы", Section::OpenQuestions),
            ("Тип встречи", Section::None),
        ] {
            assert_eq!(classify_section(heading), section, "{heading}");
        }
    }

    #[test]
    fn derives_sections_from_english_minutes() {
        let md = "\
## Summary
The team agreed on the release goal.

## Topics discussed
### Delivery risks
- Integration still needs a check.

## Key decisions
- Ship the beta on Friday.
- Not discussed

## Action items
| Owner | Task | Due |
|-------|------|-----|
| Dana | Prepare the build | Friday |

## Open questions
- Who runs the demo?";

        let structured = StructuredProtocol::from_markdown(md);

        assert_eq!(structured.title, None);
        assert_eq!(
            structured.summary,
            vec!["The team agreed on the release goal."]
        );
        assert_eq!(structured.topics.len(), 1);
        assert_eq!(structured.topics[0].title, "Delivery risks");
        assert_eq!(
            structured.topics[0].bullets,
            vec!["Integration still needs a check."]
        );
        assert_eq!(structured.decisions, vec!["Ship the beta on Friday."]);
        assert_eq!(
            structured.actions,
            vec![ProtocolAction {
                title: "Prepare the build".into(),
                owner: Some("Dana".into()),
                due: Some("Friday".into()),
            }]
        );
        assert_eq!(structured.open_questions, vec!["Who runs the demo?"]);
    }

    #[test]
    fn first_level_one_heading_still_becomes_title() {
        let structured = StructuredProtocol::from_markdown("# Планёрка\n\n## Решения\n- Го.");

        assert_eq!(structured.title.as_deref(), Some("Планёрка"));
    }
}
