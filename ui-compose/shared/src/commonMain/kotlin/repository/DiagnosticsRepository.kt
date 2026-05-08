package repository

import domain.DiagnosticsInfo

interface DiagnosticsRepository {
    suspend fun get(): DiagnosticsInfo
    suspend fun logs(): List<String>
    suspend fun openPath(path: String)
}
