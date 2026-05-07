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
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "MainKt"

        jvmArgs += listOf(
            "-Drust.target.dir=${rootProject.projectDir.parentFile}/rust/target/debug"
        )

        nativeDistributions {
            targetFormats(TargetFormat.Deb)
            packageName = "meeting-assistant"
            packageVersion = "0.1.0"
        }
    }
}
