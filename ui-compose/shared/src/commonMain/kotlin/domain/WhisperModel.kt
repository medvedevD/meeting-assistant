package domain

data class WhisperModel(
    val id: String,
    val displayName: String,
    val sizeMb: Int,
    val description: String,
    val downloadUrl: String,
)

val WHISPER_MODELS: List<WhisperModel> = listOf(
    WhisperModel(
        id = "tiny",
        displayName = "Tiny",
        sizeMb = 75,
        description = "Самая быстрая, низкая точность. Подходит для коротких фраз на английском.",
        downloadUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
    ),
    WhisperModel(
        id = "base",
        displayName = "Base",
        sizeMb = 142,
        description = "Быстрая, приемлемая точность для английского языка.",
        downloadUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
    ),
    WhisperModel(
        id = "small",
        displayName = "Small",
        sizeMb = 466,
        description = "Хороший баланс скорость/качество. Рекомендуется для русского языка.",
        downloadUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
    ),
    WhisperModel(
        id = "medium",
        displayName = "Medium (рекомендуется)",
        sizeMb = 1500,
        description = "Лучшее качество транскрипции для большинства языков. Используется по умолчанию.",
        downloadUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
    ),
    WhisperModel(
        id = "large-v3",
        displayName = "Large v3",
        sizeMb = 3100,
        description = "Максимальная точность, медленнее всего. Требует ~8 ГБ RAM при работе.",
        downloadUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
    ),
)
