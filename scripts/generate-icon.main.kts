#!/usr/bin/env kotlinc -script
// scripts/generate-icon.main.kts
// Run from repo root: kotlinc -script scripts/generate-icon.main.kts

import java.awt.Color
import java.awt.Font
import java.awt.RenderingHints
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.imageio.ImageIO

val outDir = File("ui-compose/desktopApp/src/desktopMain/resources")
outDir.mkdirs()

fun renderIcon(size: Int): BufferedImage {
    val image = BufferedImage(size, size, BufferedImage.TYPE_INT_ARGB)
    val g = image.createGraphics()
    g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON)
    g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON)
    // Dark navy circle background
    g.color = Color(30, 30, 60)
    g.fillOval(0, 0, size, size)
    // Light blue letter M
    g.color = Color(100, 180, 255)
    val fontSize = (size * 0.55).toInt()
    g.font = Font("SansSerif", Font.BOLD, fontSize)
    val fm = g.getFontMetrics()
    val text = "M"
    val x = (size - fm.stringWidth(text)) / 2
    val y = (size - fm.height) / 2 + fm.ascent
    g.drawString(text, x, y)
    g.dispose()
    return image
}

// Write PNG 512×512
ImageIO.write(renderIcon(512), "PNG", File(outDir, "icon.png"))
println("✓ icon.png (512×512)")

// Write ICO with sizes: 16, 32, 48, 256
fun writeIco(images: List<Pair<Int, BufferedImage>>, dest: File) {
    val pngBuffers = images.map { (_, img) ->
        val baos = ByteArrayOutputStream()
        ImageIO.write(img, "PNG", baos)
        baos.toByteArray()
    }

    val headerSize = 6
    val dirEntrySize = 16
    val dirSize = images.size * dirEntrySize
    val dataOffset = headerSize + dirSize

    val buf = ByteArrayOutputStream()
    fun writeShort(v: Int) = buf.write(ByteBuffer.allocate(2).order(ByteOrder.LITTLE_ENDIAN).putShort(v.toShort()).array())
    fun writeInt(v: Int) = buf.write(ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(v).array())

    writeShort(0)
    writeShort(1)
    writeShort(images.size)

    var offset = dataOffset
    for ((i, pair) in images.withIndex()) {
        val (size, _) = pair
        val pngBytes = pngBuffers[i]
        buf.write(if (size == 256) 0 else size)
        buf.write(if (size == 256) 0 else size)
        buf.write(0)
        buf.write(0)
        writeShort(1)
        writeShort(32)
        writeInt(pngBytes.size)
        writeInt(offset)
        offset += pngBytes.size
    }

    for (pngBytes in pngBuffers) {
        buf.write(pngBytes)
    }

    dest.writeBytes(buf.toByteArray())
}

val icoSizes = listOf(16, 32, 48, 256)
val icoImages = icoSizes.map { it to renderIcon(it) }
writeIco(icoImages, File(outDir, "icon.ico"))
println("✓ icon.ico (16, 32, 48, 256 px)")
println("Done! Icons written to: ${outDir.absolutePath}")
