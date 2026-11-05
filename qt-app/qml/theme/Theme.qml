// Meety design tokens — single source of truth for the redesigned UI.
// Port of the design's CSS `:root` tokens. CSS uses oklch();
// QML is sRGB-only, so the palette below is the light+sienna set converted to
// hex once. Screens reference `Theme.*`
// instead of `palette.*`.
//
// Scope (decided): light + sienna only. Dark / slate / moss / density variants
// are intentionally NOT implemented yet — the structure here (flat readonly
// tokens + a `dark` flag reserved below) lets them be added later without
// touching screens.
pragma Singleton
import QtQuick

QtObject {
    id: theme

    // ── fonts ────────────────────────────────────────────────────────────────
    // Variable TTFs bundled in qt-app/fonts/, registered as :/fonts/* resources.
    // The FontLoaders register the families; consumers use the string names and
    // set font.weight per use (the files are variable, covering 400–700).
    readonly property FontLoader _ui: FontLoader {
        source: "qrc:/fonts/Geist-Variable.ttf"
    }
    readonly property FontLoader _serif: FontLoader {
        source: "qrc:/fonts/Newsreader-Variable.ttf"
    }
    readonly property FontLoader _serifItalic: FontLoader {
        source: "qrc:/fonts/Newsreader-Italic-Variable.ttf"
    }
    readonly property FontLoader _serifCyrillic: FontLoader {
        source: "qrc:/fonts/SourceSerif4-Variable.ttf"
    }
    readonly property FontLoader _serifCyrillicItalic: FontLoader {
        source: "qrc:/fonts/SourceSerif4-Italic-Variable.ttf"
    }
    readonly property FontLoader _mono: FontLoader {
        source: "qrc:/fonts/JetBrainsMono-Variable.ttf"
    }

    readonly property string fontUi: _ui.name.length > 0 ? _ui.name : "Geist"
    readonly property string fontMono: _mono.name.length > 0 ? _mono.name : "JetBrains Mono"

    // The serif for a piece of text. Newsreader has no Cyrillic, and Qt would set
    // the missing letters in a system font while digits and Latin words in the same
    // line stay in Newsreader. So a string with Cyrillic uses Source Serif 4 whole,
    // and Newsreader keeps everything else.
    function serifFor(text) {
        return /[\u0400-\u052F]/.test(text) ? _sourceSerifName : _newsreaderName
    }
    readonly property string _newsreaderName: _serif.name.length > 0 ? _serif.name : "Newsreader"
    readonly property string _sourceSerifName: _serifCyrillic.name.length > 0 ? _serifCyrillic.name
                                                                              : "Source Serif 4"

    // How to draw serif text in `family`. Source Serif 4 has an optical size axis,
    // and macOS picks the cut from the font size. Qt's default renderer takes every
    // glyph from one large master, the display cut, but spaces it for the real size,
    // which leaves thin letters with uneven gaps. Native rendering draws the cut the
    // layout measured. Other systems don't pick a cut by size, so they stay on the
    // default. Newsreader has the same axis and keeps its current look on purpose.
    function renderTypeFor(family) {
        return family === _sourceSerifName && Qt.platform.os === "osx"
                ? Text.NativeRendering : Text.QtRendering
    }

    // weights (variable fonts cover the range)
    readonly property int wRegular: Font.Normal      // 400
    readonly property int wMedium: Font.Medium       // 500
    readonly property int wSemiBold: Font.DemiBold   // 600
    readonly property int wBold: Font.Bold           // 700

    // ── palette (light · sienna) ──────────────────────────────────────────────
    readonly property bool dark: false   // reserved for the future dark set

    readonly property color paper: "#FBF9F5"      // primary bg
    readonly property color paperSub: "#F5F1EC"   // sidebar / card bg
    readonly property color paper3: "#EDE9E2"     // hover / chip
    readonly property color paper4: "#E3DDD5"     // selected

    readonly property color ink: "#1F1A15"        // primary text
    readonly property color ink2: "#47413C"       // secondary text
    readonly property color ink3: "#77706A"       // tertiary / muted
    readonly property color ink4: "#A9A49E"       // placeholder

    readonly property color rule: "#DBD7D0"       // hairline
    readonly property color rule2: "#C9C3BC"

    readonly property color accent: "#C45E3D"     // burnt sienna
    readonly property color accent2: "#B84221"    // hover
    readonly property color accentTint: "#FFE3D8" // soft fill
    readonly property color accentInk: "#FFFFFF"  // on-accent text
    readonly property color focus: theme.ink3
    readonly property color focusTint: theme.paper3

    readonly property color rec: "#DF202E"        // recording dot
    readonly property color ok: "#2E9052"
    readonly property color warn: "#CD9130"

    // ── radii ─────────────────────────────────────────────────────────────────
    readonly property int rSm: 6
    readonly property int rMd: 8
    readonly property int rLg: 12
    readonly property int rXl: 16

    // ── spacing (density: regular) ─────────────────────────────────────────────
    readonly property int rowPy: 10
    readonly property int gap: 16
    readonly property int sidebarWidth: 268
    readonly property int sidebarCompactWidth: 88

    // ── type scale (px sizes used across the design) ───────────────────────────
    readonly property int fsMicro: 11      // section labels, meta
    readonly property int fsSmall: 12
    readonly property int fsBody: 13        // list rows, menu, buttons
    readonly property int fsBodyLg: 14      // inputs, card body
    readonly property int fsTitle: 22       // brand, content-title (serif)
    readonly property int fsH1: 30          // newrec / welcome headings (serif)
    readonly property int fsDisplay: 44     // protocol h1 (serif)

    // ── animation durations (ms) ────────────────────────────────────────────────
    readonly property int durFast: 80
    readonly property int durBase: 120
    readonly property int durSlow: 300

    // Letter-spacing helper: CSS tracking is in `em`; QML font.letterSpacing is
    // in pixels. tracking(size, em) → pixels.
    function tracking(sizePx, em) { return sizePx * em }
}
