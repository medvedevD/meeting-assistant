import java.util.prefs.Preferences

object WindowPrefs {
    private val prefs: Preferences = Preferences.userRoot().node("meeting-assistant/window")

    var width: Int
        get() = prefs.getInt("width", 1280)
        set(v) { prefs.putInt("width", v) }

    var height: Int
        get() = prefs.getInt("height", 800)
        set(v) { prefs.putInt("height", v) }

    // INT_MIN sentinel = "not set"
    private const val UNSET = Int.MIN_VALUE

    var x: Int?
        get() = prefs.getInt("x", UNSET).takeUnless { it == UNSET }
        set(v) { if (v != null) prefs.putInt("x", v) else prefs.remove("x") }

    var y: Int?
        get() = prefs.getInt("y", UNSET).takeUnless { it == UNSET }
        set(v) { if (v != null) prefs.putInt("y", v) else prefs.remove("y") }

    fun save(width: Int, height: Int, x: Int?, y: Int?) {
        this.width = width
        this.height = height
        this.x = x
        this.y = y
        prefs.flush()
    }
}
