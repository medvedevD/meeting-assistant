// Generates the Finder background image for the Meeting Assistant install DMG.
//
// Gradient backdrop + short launch steps baked into the image (there is no
// separate "read me" file — the steps live right in the window). The icons
// (app + Applications) sit in a row near the TOP; all text sits BELOW them, so
// even when macOS 26 Finder opens the window larger than requested (it does not
// reliably honor a DMG's saved window size), the absolute-positioned icons stay
// up top and the stretched background text stays lower — they don't collide. We
// deliberately avoid an arrow keyed to an icon position (that WOULD skew on a
// resized window); the icon row + the cue text convey the drag.
//
// Output is committed (packaging/assets/dmg-background.png) so the release build
// needs no Swift toolchain. Regenerate after editing:
//   swiftc packaging/macos/make-dmg-background.swift -o /tmp/mkbg \
//     && /tmp/mkbg packaging/assets/dmg-background.png
import AppKit

let outPath = CommandLine.arguments.count > 1
    ? CommandLine.arguments[1] : "dmg-background.png"

// Finder/create-dmg treats background pixels as window points instead of
// applying Retina @2x scaling. Keep the bitmap exactly equal to the requested
// 720x480 window or Finder crops the lower/right half and hides the guide.
// Icon positions live in build-app.sh and keep the icon row near y≈150.
let S: CGFloat = 1
let W: CGFloat = 720 * S
let H: CGFloat = 480 * S

let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(W), pixelsHigh: Int(H),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
let nsctx = NSGraphicsContext(bitmapImageRep: rep)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = nsctx
let ctx = nsctx.cgContext

// ── Soft vertical gradient (off-white → light cool gray) ─────────────────────
let top = CGColor(red: 0.97, green: 0.975, blue: 0.985, alpha: 1)
let bot = CGColor(red: 0.89, green: 0.905, blue: 0.925, alpha: 1)
let grad = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(),
                      colors: [top, bot] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: H),
                       end: CGPoint(x: 0, y: 0), options: [])

// Helpers (AppKit context is bottom-left origin; place text by its top-down y).
func fromTop(_ y: CGFloat) -> CGFloat { H - y * S }
let cx = W / 2
let accent = NSColor(red: 0, green: 0.45, blue: 0.95, alpha: 1)

func draw(_ s: String, top yTop: CGFloat, size: CGFloat, weight: NSFont.Weight,
          color: NSColor) {
    let font = NSFont.systemFont(ofSize: size * S, weight: weight)
    let str = NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: color])
    let sz = str.size()
    str.draw(at: CGPoint(x: cx - sz.width / 2, y: fromTop(yTop) - sz.height))
}

let dark = NSColor(white: 0.13, alpha: 1)
let gray = NSColor(white: 0.34, alpha: 1)

// ── Drag cue (just below the app + Applications icon row at y≈150) ────────────
draw("Перетащите значок в Applications и откройте приложение",
     top: 235, size: 13.5, weight: .medium, color: gray)

// ── First-launch steps (the Gatekeeper "Open Anyway" path) ───────────────────
draw("Первый запуск — если macOS пишет «не удалось проверить»:",
     top: 300, size: 13, weight: .semibold, color: dark)
draw("Откройте Системные настройки → Конфиденциальность и безопасность",
     top: 330, size: 12.5, weight: .regular, color: dark)
draw("и нажмите «Всё равно открыть» (Open Anyway)",
     top: 353, size: 12.5, weight: .semibold, color: accent)

NSGraphicsContext.restoreGraphicsState()

// ── Save PNG ──────────────────────────────────────────────────────────────────
guard let png = rep.representation(using: .png, properties: [:]) else {
    FileHandle.standardError.write(Data("error: could not encode PNG\n".utf8))
    exit(1)
}
do {
    try png.write(to: URL(fileURLWithPath: outPath))
    print("wrote \(outPath) (\(Int(W))x\(Int(H)))")
} catch {
    FileHandle.standardError.write(Data("error: \(error)\n".utf8))
    exit(1)
}
