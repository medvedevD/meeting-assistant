// Maps a sidecar `error_class` (snake_case, see core entities/job.rs ErrorClass)
// to a localized, user-facing message + whether the error is fixable in
// Settings (drives an "Открыть настройки" affordance). Imported as a JS library:
//   import "../i18n/errors.js" as Errors
.pragma library

// Returns { title, hint, settings } for an error_class. Falls back to the raw
// server message for unknown / null classes.
function describe(errorClass, rawMessage) {
    switch (errorClass) {
    case "api_auth":
        return {
            title: "LLM не настроена",
            hint: "API-ключ не задан или неверен. Укажите ключ выбранного провайдера в настройках.",
            settings: true
        }
    case "api_quota":
        return {
            title: "Превышена квота API",
            hint: "У провайдера закончились кредиты или достигнут лимит запросов. Проверьте баланс или смените провайдера в настройках.",
            settings: true
        }
    case "network_timeout":
        return {
            title: "Сетевая ошибка",
            hint: "Не удалось связаться с провайдером. Проверьте подключение к интернету и базовый URL в настройках.",
            settings: true
        }
    case "audio_corrupt":
        return {
            title: "Не удалось прочитать аудио",
            hint: "Файл повреждён или имеет неподдерживаемый формат. Попробуйте другой файл.",
            settings: false
        }
    case "model_missing":
        return {
            title: "Модель Whisper не найдена",
            hint: "Укажите корректный путь к файлу модели в настройках транскрайбера.",
            settings: true
        }
    case "model_not_selected":
        return {
            title: "Модель Whisper не выбрана",
            hint: "Выберите модель транскрипции в настройках.",
            settings: true
        }
    case "worker_killed":
        return {
            title: "Обработка прервана",
            hint: "Процесс обработки был остановлен. Попробуйте ещё раз.",
            settings: false
        }
    case "cancelled":
        return {
            title: "Отменено",
            hint: "Задача прервана по вашему запросу. Запустите её снова, если нужно.",
            settings: false,
            neutral: true
        }
    case "unknown":
    default:
        return {
            title: "Не удалось обработать встречу",
            hint: rawMessage && rawMessage.length > 0
                  ? rawMessage
                  : "Произошла непредвиденная ошибка.",
            settings: false
        }
    }
}
