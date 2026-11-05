// Maps a sidecar `error_class` (snake_case, see core entities/job.rs ErrorClass)
// to a localized, user-facing message + whether the error is fixable in
// Settings (drives an "Открыть настройки" affordance). Imported as a JS library:
//   import "../i18n/errors.js" as Errors
// qsTr works here: in a .pragma library it takes its context from this file's
// name, which is the context lupdate records ("errors").
.pragma library

// Returns { title, hint, settings, detail } for an error_class. `detail` is the
// raw server message (for LLM errors, the provider's own reason, e.g. "This
// model is not available in your subscription tier") shown under the hint so
// the user sees the actual cause. Unknown / null classes have no localized
// copy, so the raw message becomes the hint itself and `detail` stays empty.
function describe(errorClass, rawMessage) {
    var d = _describe(errorClass, rawMessage)
    var known = errorClass && errorClass !== "unknown" && d.neutral !== true
    d.detail = known && rawMessage ? String(rawMessage) : ""
    return d
}

function _describe(errorClass, rawMessage) {
    switch (errorClass) {
    case "api_auth":
        return {
            title: qsTr("LLM не настроена"),
            hint: qsTr("API-ключ не задан или неверен. Укажите ключ выбранного провайдера в настройках."),
            settings: true
        }
    case "llm_model_unavailable":
        return {
            title: qsTr("Модель недоступна"),
            hint: qsTr("Провайдер отказал в доступе к выбранной модели — обычно её нет в вашем тарифе, название указано с ошибкой или неверен базовый URL. Выберите другую модель в настройках."),
            settings: true
        }
    case "llm_rejected":
        return {
            title: qsTr("Провайдер отклонил запрос"),
            hint: qsTr("Проверьте параметры модели в настройках — например, максимум токенов ответа."),
            settings: true
        }
    case "api_quota":
        return {
            title: qsTr("Превышена квота API"),
            hint: qsTr("У провайдера закончились кредиты или достигнут лимит запросов. Проверьте баланс или смените провайдера в настройках."),
            settings: true
        }
    case "network_timeout":
        return {
            title: qsTr("Сетевая ошибка"),
            hint: qsTr("Не удалось связаться с провайдером. Проверьте подключение к интернету и базовый URL в настройках."),
            settings: true
        }
    case "audio_corrupt":
        return {
            title: qsTr("Не удалось прочитать аудио"),
            hint: qsTr("Файл повреждён или имеет неподдерживаемый формат. Попробуйте другой файл."),
            settings: false
        }
    case "model_missing":
        return {
            title: qsTr("Модель Whisper не найдена"),
            hint: qsTr("Укажите корректный путь к файлу модели в настройках транскрайбера."),
            settings: true
        }
    case "model_not_selected":
        return {
            title: qsTr("Модель Whisper не выбрана"),
            hint: qsTr("Выберите модель транскрипции в настройках."),
            settings: true
        }
    case "worker_killed":
        return {
            title: qsTr("Обработка прервана"),
            hint: qsTr("Процесс обработки был остановлен. Попробуйте ещё раз."),
            settings: false
        }
    case "cancelled":
        return {
            title: qsTr("Отменено"),
            hint: qsTr("Задача прервана по вашему запросу. Запустите её снова, если нужно."),
            settings: false,
            neutral: true
        }
    case "unknown":
    default:
        return {
            title: qsTr("Не удалось обработать встречу"),
            hint: rawMessage && rawMessage.length > 0
                  ? rawMessage
                  : qsTr("Произошла непредвиденная ошибка."),
            settings: false
        }
    }
}
