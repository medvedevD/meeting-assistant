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
                implementation(compose.materialIconsExtended)
                implementation(libs.decompose)
                implementation(libs.decompose.extensions.compose)
                implementation(libs.kotlinx.coroutines.core)
            }
        }
        val desktopMain by getting {
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation(libs.jna)
                implementation(libs.jna.platform)
                implementation(libs.kotlinx.coroutines.swing)
            }
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test-junit5"))
                implementation("org.junit.jupiter:junit-jupiter:5.10.2")
                implementation(libs.kotlinx.coroutines.core)
                implementation(libs.jna)
                implementation(libs.jna.platform)
            }
        }
    }
}

tasks.named<Test>("desktopTest") {
    useJUnitPlatform()
    systemProperty(
        "rust.target.dir",
        System.getProperty("rust.target.dir")
            ?: "${rootProject.projectDir.parentFile}/rust/target/debug"
    )
}
