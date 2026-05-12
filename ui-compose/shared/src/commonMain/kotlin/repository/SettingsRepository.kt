package repository

import domain.ModelInfo
import domain.Settings

interface SettingsRepository {
    suspend fun get(): Settings
    suspend fun set(settings: Settings)
    suspend fun templatesList(): List<String>
    suspend fun modelsList(): List<ModelInfo>
}
