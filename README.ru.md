<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/brand/logo-light.svg">
    <img alt="Meeting Assistant" src="docs/brand/logo-light.svg" width="520">
  </picture>
</h1>

[![CI](https://github.com/medvedevD/meeting-assistant/actions/workflows/qt-ci.yml/badge.svg?branch=main)](https://github.com/medvedevD/meeting-assistant/actions/workflows/qt-ci.yml)
[![Release](https://img.shields.io/github/v/release/medvedevD/meeting-assistant?include_prereleases&label=release)](https://github.com/medvedevD/meeting-assistant/releases)
[![License: MIT](https://img.shields.io/github/license/medvedevD/meeting-assistant)](LICENSE)

[English](README.md) | **Русский**

Запишите встречу и получите расшифровку и готовый протокол. Речь распознаётся прямо на
вашем компьютере, а протокол пишет выбранная вами нейросеть.

> **Alpha.** Сборки пока не подписаны коммерческими сертификатами, поэтому при первом
> запуске macOS и Windows покажут предупреждение.

## Возможности

- **Любой источник:** микрофон, системный звук или оба сразу; можно загрузить готовый файл.
- **Расшифровка без облака:** Whisper работает локально, аудио не покидает компьютер.
- **Протоколы по шаблонам:** дейлики, встречи 1-на-1, командные встречи; шаблоны можно менять.
- **Любая нейросеть:** Anthropic, OpenAI, Gemini или Mistral с вашим ключом либо Ollama офлайн.
- **История встреч:** записи, расшифровки и протоколы хранятся локально; встречу можно переслушать или обработать заново.

## Скачать

Последняя сборка для macOS, Windows и Linux лежит в
[Releases](https://github.com/medvedevD/meeting-assistant/releases). На macOS можно
установить через Homebrew:

```bash
brew tap medvedevd/meeting-assistant && brew install --cask meeting-assistant
```

Как пройти предупреждения при первом запуске и настроить приложение, описано в
[инструкции по установке](docs/INSTALL.ru.md).

## Приватность

Аудио обрабатывается только на вашем компьютере. Расшифровка уходит выбранному вами
LLM-провайдеру и только при создании протокола. С Ollama данные вообще не покидают
компьютер. API-ключи хранятся в системном хранилище ключей, если оно доступно.

## Как это устроено

**Стек:** Rust, Qt 6 (QML и C++), whisper.cpp, SQLite, Axum.

- **Два процесса:** интерфейс на Qt запускает sidecar на Rust и общается с ним по локальному HTTP с токеном на каждый запуск, поэтому сбой в нативном коде не роняет интерфейс.
- **Чистая архитектура:** доменная логика зависит только от интерфейсов, а Whisper, аудио, хранилище и LLM-провайдеры подключаются как адаптеры.
- **Надёжные задачи:** очередь в SQLite сохраняет результат до завершения задачи, поэтому повторная попытка не запускает Whisper и платный запрос к LLM заново.
- **Нативный звук в каждой ОС:** ScreenCaptureKit на macOS, WASAPI loopback на Windows, PulseAudio/PipeWire на Linux.
- **Релизы с проверками:** тег версии собирает установщики только после проверки протокола, форматирования, Clippy и тестов.

Подробнее в [описании архитектуры](docs/ARCHITECTURE.md). Как собрать и запустить проект, описано в [руководстве разработчика](docs/DEVELOPMENT.md). Оба документа на английском.

## Карта репозитория

| Путь | Содержимое |
|---|---|
| `rust/` | Ядро, адаптеры, HTTP API и sidecar |
| `qt-app/` | Интерфейс на QML и C++ с тестами |
| `prompts/` | Встроенные шаблоны протоколов |
| `packaging/` | Сборка установщиков, лицензии и чек-лист релиза |
| `docs/` | Установка, разработка, архитектура и логотипы |
| `plans/`, `backlog/` | Текущие задачи и идеи на будущее |

## Лицензия

[MIT](LICENSE). В готовые сборки также входят Qt и FFmpeg под LGPL, см.
[`packaging/LICENSES`](packaging/LICENSES).
