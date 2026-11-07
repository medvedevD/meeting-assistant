# Установка

[English](INSTALL.md) | **Русский**

Готовые сборки лежат в
[GitHub Releases](https://github.com/medvedevD/meeting-assistant/releases): для macOS на
Apple silicon, Windows x64 и Linux x86_64. Это alpha-сборки без коммерческих
сертификатов Apple и Microsoft, поэтому при первом запуске macOS и Windows покажут
предупреждение.

Сборкам для Windows и Linux нужен процессор с AVX2: Intel Haswell (2013) или AMD
Excavator (2015) и новее. У некоторых Pentium, Celeron и Atom его нет. На таком
процессоре приложение откроется, но транскрипция завершится ошибкой.

## macOS

Рекомендуемый способ, через Homebrew:

```bash
brew tap medvedevd/meeting-assistant
```

```bash
brew install --cask meeting-assistant
```

Если Homebrew попросит доверять стороннему tap, выполните указанную им команду
`brew trust` и повторите установку.

Обновление:

```bash
brew upgrade --cask meeting-assistant
```

Также можно скачать `MeetingAssistant-*.dmg` из Releases:

1. Откройте DMG и перетащите MeetingAssistant в Applications.
2. Запустите приложение из Applications.
3. Если macOS заблокирует первый запуск, откройте
   **System Settings → Privacy & Security** и нажмите **Open Anyway**.

DMG подписан ad-hoc, но не нотарифицирован Apple. Полностью убрать предупреждение можно
только после перехода на Developer ID и нотарификацию.

## Windows

Скачайте `MeetingAssistant-Setup-*.exe` из Releases и запустите установщик. Поскольку
сборка не подписана, SmartScreen может потребовать выбрать
**More info → Run anyway**.

## Linux

Скачайте AppImage, сделайте его исполняемым и запустите:

```bash
chmod +x MeetingAssistant-*.AppImage
```

```bash
./MeetingAssistant-*.AppImage
```

## Первый запуск

1. Разрешите приложению доступ к микрофону. На macOS для записи системного звука также
   потребуется разрешение Screen Recording.
2. Выберите или добавьте модель Whisper в настройках.
3. Настройте LLM-провайдера. Для облачных провайдеров нужен собственный API-ключ, а
   Ollama работает локально без ключа.
4. Создайте запись, дождитесь расшифровки и сформируйте протокол.

## Логи

Если что-то пошло не так, журнал приложения находится здесь:

- **macOS:** `~/Library/Application Support/meeting-assistant/logs/meeting-assistant.log`
- **Linux:** `~/.local/share/meeting-assistant/logs/meeting-assistant.log`
- **Windows:** `%LOCALAPPDATA%\meeting-assistant\logs\meeting-assistant.log`
