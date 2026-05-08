package repository

import domain.Settings

interface SettingsRepository {
    suspend fun get(): Settings
    suspend fun set(settings: Settings)
    suspend fun templatesList(): List<String>
}
