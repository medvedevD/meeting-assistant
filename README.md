# Meeting Assistant

Десктопное приложение для записи встреч, локальной расшифровки речи и
подготовки протоколов с помощью LLM.

> Проект находится на стадии alpha. Сборки предназначены для тестирования и
> пока не подписаны коммерческими сертификатами Apple и Microsoft.

## Возможности

- запись микрофона, системного звука или смешанного источника;
- локальная транскрипция через Whisper;
- генерация протокола по редактируемым шаблонам;
- Anthropic, OpenAI, Gemini, Mistral и локальная Ollama;
- история встреч, повторная обработка и встроенное воспроизведение записей;
- хранение данных и настроек на компьютере пользователя.

## Установка

Готовые сборки находятся в
[GitHub Releases](https://github.com/medvedevD/meeting-assistant/releases).

### macOS

Рекомендуемый бесплатный канал:

```bash
brew tap medvedevd/meeting-assistant
brew install --cask meeting-assistant
```

Если Homebrew попросит доверять стороннему tap, выполните указанную им команду
`brew trust`, затем повторите установку.

Обновление:

```bash
brew upgrade --cask meeting-assistant
```

Также можно скачать `MeetingAssistant-*.dmg` из Releases:

1. Откройте DMG и перетащите MeetingAssistant в Applications.
2. Запустите приложение из Applications.
3. Если macOS заблокирует первый запуск, откройте
   **System Settings → Privacy & Security** и нажмите **Open Anyway**.

DMG подписан ad-hoc, но пока не нотарифицирован Apple. Полностью убрать
Gatekeeper-предупреждение можно только после перехода на Developer ID и
notarization.

### Windows

Скачайте `MeetingAssistant-Setup-*.exe` из Releases и запустите установщик.
Поскольку alpha-сборка пока не подписана, SmartScreen может потребовать выбрать
**More info → Run anyway**.

### Linux

Скачайте AppImage и сделайте его исполняемым:

```bash
chmod +x MeetingAssistant-*.AppImage
./MeetingAssistant-*.AppImage
```

## Первый запуск

1. Разрешите приложению доступ к микрофону. На macOS для записи системного
   звука также потребуется разрешение Screen Recording.
2. Выберите или добавьте модель Whisper в настройках.
3. Настройте LLM-провайдера. Для облачных провайдеров нужен собственный
   API-ключ; Ollama работает локально без ключа.
4. Создайте запись, дождитесь транскрипции и сформируйте протокол.

Аудио обрабатывается Whisper локально и не отправляется в облако. Текст
транскрипции передаётся наружу только выбранному пользователем LLM-провайдеру
при генерации протокола.

## Разработка

Требуются Rust, CMake и Qt 6.8+ с модулями Quick, Network, Multimedia и Svg.

Основной dev workflow:

```bash
./run-qt.sh
```

Полезные варианты:

```bash
./run-qt.sh --debug
./run-qt.sh --no-run
FIRST_RUN=1 ./run-qt.sh
```

Тесты:

```bash
cargo test --manifest-path rust/Cargo.toml
ctest --test-dir qt-app/build
```

## Архитектура

- `rust/` — Rust core, адаптеры, HTTP API и sidecar `meeting-server`;
- `qt-app/` — Qt 6 / QML интерфейс;
- `prompts/` — встроенные шаблоны протоколов;
- `packaging/` — сборка установочных артефактов для macOS, Linux и Windows.

GUI запускает sidecar рядом со своим executable и взаимодействует с ним по
локальному HTTP с одноразовым токеном.

## Диагностика

Логи приложения:

- macOS: `~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log`
- Linux: `~/.local/share/meeting-assistant/logs/meeting-assistant.log`
- Windows: `%LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log`
