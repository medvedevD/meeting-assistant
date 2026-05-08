package ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val primaryDark = Color(0xFF4FA3E0)
private val onPrimaryDark = Color(0xFF003355)
private val surfaceDark = Color(0xFF1E2027)
private val backgroundDark = Color(0xFF16181E)
private val onSurfaceDark = Color(0xFFE2E4EC)
private val surfaceVariantDark = Color(0xFF282C36)

private val DarkColors = darkColorScheme(
    primary = primaryDark,
    onPrimary = onPrimaryDark,
    surface = surfaceDark,
    onSurface = onSurfaceDark,
    background = backgroundDark,
    onBackground = onSurfaceDark,
    surfaceVariant = surfaceVariantDark,
    onSurfaceVariant = Color(0xFFB0B4C0),
)

private val LightColors = lightColorScheme(
    primary = Color(0xFF1A6FAF),
    onPrimary = Color(0xFFFFFFFF),
    surface = Color(0xFFF5F6FA),
    onSurface = Color(0xFF1A1C22),
    background = Color(0xFFEEF0F5),
    onBackground = Color(0xFF1A1C22),
)

@Composable
fun AppTheme(
    darkTheme: Boolean = true,
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content,
    )
}
