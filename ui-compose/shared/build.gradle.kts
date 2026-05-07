plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.compiler)
}

kotlin {
    jvm("desktop")

    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(compose.runtime)
                implementation(compose.foundation)
                implementation(compose.material3)
            }
        }
        val desktopMain by getting {
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation(libs.jna)
                implementation(libs.jna.platform)
            }
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test-junit5"))
                implementation("org.junit.jupiter:junit-jupiter:5.10.2")
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")
                implementation(libs.jna)
                implementation(libs.jna.platform)
            }
        }
    }
}

tasks.named<Test>("desktopTest") {
    useJUnitPlatform()
    // Pass the Rust target dir so the test can load the native library.
    systemProperty(
        "rust.target.dir",
        System.getProperty("rust.target.dir")
            ?: "${rootProject.projectDir.parentFile}/rust/target/debug"
    )
}
