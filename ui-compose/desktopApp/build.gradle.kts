import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.compiler)
}

kotlin {
    jvm("desktop")

    sourceSets {
        val desktopMain by getting {
            dependencies {
                implementation(project(":shared"))
                implementation(compose.desktop.currentOs)
                implementation(compose.material3)
                implementation(libs.decompose)
                implementation(libs.kotlinx.coroutines.swing)
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "MainKt"

        // Dev: передаём путь к .so через JVM property (используется как fallback в Main.kt)
        val rustTargetDir = System.getProperty("rust.target.dir")
            ?: "${rootProject.projectDir.parentFile}/rust/target/release"
        jvmArgs += listOf("-Drust.target.dir=$rustTargetDir")

        // Dev: путь к prompts/ репозитория (в бандле берётся из resources — см. copyPrompts)
        jvmArgs += listOf("-Dmeeting.prompts.dir=${rootProject.projectDir.parentFile}/prompts")

        // ProGuard обфусцирует com.sun.jna.* и uniffi-биндинги, которые JNA
        // резолвит по именам через JNI (Native.initIDs ищет `dispose` и т.п.) →
        // UnsatisfiedLinkError на старте release-сборки. Отключаем минификацию:
        // экономия размера ничтожна на фоне whisper-библиотек и моделей.
        buildTypes.release.proguard {
            isEnabled.set(false)
        }

        nativeDistributions {
            // Native libs кладём в resources/<os>/ — Compose Desktop включает их в бандл
            appResourcesRootDir.set(project.layout.projectDirectory.dir("resources"))

            targetFormats(TargetFormat.Deb, TargetFormat.Msi, TargetFormat.Dmg)
            packageName = "meeting-assistant"
            packageVersion = System.getProperty("app.version") ?: "1.0.0"
            description = "AI-powered meeting assistant"
            vendor = "Meeting Assistant"
            copyright = "© 2025 Meeting Assistant"

            linux {
                iconFile.set(project.file("src/desktopMain/resources/icon.png"))
                packageName = "meeting-assistant"
                debMaintainer = "codemedvedev@gmail.com"
                menuGroup = "Productivity"
                appCategory = "Utility"
            }

            windows {
                iconFile.set(project.file("src/desktopMain/resources/icon.ico"))
                menuGroup = "Meeting Assistant"
                // НЕ МЕНЯТЬ после первого релиза — используется для upgrade detection
                upgradeUuid = "A1B2C3D4-E5F6-7890-ABCD-EF1234567890"
                perUserInstall = true
                shortcut = true
            }

            macOS {
                iconFile.set(project.file("src/desktopMain/resources/icon.icns"))
                bundleID = "com.meeting-assistant.app"
                minimumSystemVersion = "12.0"
                // codesign/notarization — отложено (нет Apple Developer ID)
            }
        }
    }
}

// Пути к native libs (передаются через -Drust.target.dir или дефолтный release)
val nativeLibsRustDir: String = System.getProperty("rust.target.dir")
    ?: "${project.projectDir.parentFile.parentFile}/rust/target/release"

// DefaultTask вместо Copy — иначе Gradle пропускает задачу как NO-SOURCE при отсутствии файла,
// и doFirst (с проверкой существования) никогда не вызывается.
tasks.register("copyNativeLibLinux") {
    val src = File(nativeLibsRustDir, "libmeeting_assistant_ffi.so")
    val dst = project.layout.projectDirectory.dir("resources/linux")
    doFirst {
        require(src.exists()) {
            "libmeeting_assistant_ffi.so not found at $src\nRun: ./build-ffi.sh"
        }
    }
    doLast {
        copy { from(src); into(dst) }
    }
}

tasks.register("copyNativeLibWindows") {
    val src = File(nativeLibsRustDir, "meeting_assistant_ffi.dll")
    val dst = project.layout.projectDirectory.dir("resources/windows")
    doFirst {
        require(src.exists()) {
            "meeting_assistant_ffi.dll not found at $src\nRun: cargo build --release -p ffi"
        }
    }
    doLast {
        copy { from(src); into(dst) }
    }
}

tasks.register("copyNativeLibMacos") {
    val src = File(nativeLibsRustDir, "libmeeting_assistant_ffi.dylib")
    val dst = project.layout.projectDirectory.dir("resources/macos")
    doFirst {
        require(src.exists()) {
            "libmeeting_assistant_ffi.dylib not found at $src\nRun: cargo build --release -p ffi"
        }
    }
    doLast {
        copy { from(src); into(dst) }
    }
}

// Промпт-шаблоны OS-независимы → resources/common (Compose Desktop кладёт их
// в один resources.dir, рядом с native libs). Без этого packaged-сборка падает
// с "template error: No such file or directory" при открытии окна настроек.
tasks.register("copyPrompts") {
    val src = File("${rootProject.projectDir.parentFile}/prompts")
    val dst = project.layout.projectDirectory.dir("resources/common/prompts")
    doFirst {
        require(src.exists() && src.isDirectory) {
            "prompts/ not found at $src"
        }
    }
    doLast {
        project.delete(dst)                       // выкидываем устаревшие шаблоны
        copy { from(src); into(dst) }
    }
}

// ВАЖНО: внутри afterEvaluate — Compose Desktop регистрирует packaging-задачи в afterEvaluate.
// tasks.named() вне afterEvaluate бросит UnknownTaskException.
// tasks.findByName() безопаснее чем tasks.named() — не падает если таск не зарегистрирован на данной ОС.
afterEvaluate {
    listOf("packageDeb", "packageReleaseDeb").forEach { name ->
        tasks.findByName(name)?.dependsOn("copyNativeLibLinux", "copyPrompts")
    }
    listOf("packageMsi", "packageReleaseMsi").forEach { name ->
        tasks.findByName(name)?.dependsOn("copyNativeLibWindows", "copyPrompts")
    }
    listOf("packageDmg", "packageReleaseDmg").forEach { name ->
        tasks.findByName(name)?.dependsOn("copyNativeLibMacos", "copyPrompts")
    }

    // `run`/`runDistributable` resolve the native lib from the prepared resources
    // dir (Main.kt reads compose.application.resources.dir). prepareAppResources
    // only copies what's already in resources/<os>/, so the OS-specific native
    // lib must be staged there first — wire it here so dev runs work, not just packaging.
    val os = org.gradle.internal.os.OperatingSystem.current()
    val nativeLibCopyTask = when {
        os.isMacOsX -> "copyNativeLibMacos"
        os.isWindows -> "copyNativeLibWindows"
        else -> "copyNativeLibLinux"
    }
    tasks.findByName("prepareAppResources")
        ?.dependsOn(nativeLibCopyTask, "copyPrompts")
}
