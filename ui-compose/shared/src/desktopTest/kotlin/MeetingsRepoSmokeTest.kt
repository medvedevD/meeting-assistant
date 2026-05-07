import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.*
import uniffi.meeting_assistant_ffi.*
import java.io.File

/**
 * Smoke test that calls every UniFFI-exported method on AppCore.
 *
 * Requirements to run (skipped otherwise):
 *   - MEETING_ASSISTANT_MODEL env var → path to a whisper .bin model file
 *   - rust.target.dir system property → path to Rust debug build dir
 *       (set automatically by Gradle task config, or -Drust.target.dir=...)
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation::class)
class MeetingsRepoSmokeTest {

    companion object {
        private var core: AppCore? = null
        private var workerHandle: WorkerHandle? = null

        @BeforeAll
        @JvmStatic
        fun setup() {
            val modelPath = System.getenv("MEETING_ASSISTANT_MODEL")
                ?: run {
                    println("[SKIP] MEETING_ASSISTANT_MODEL not set")
                    return
                }
            if (!File(modelPath).exists()) {
                println("[SKIP] Model file not found: $modelPath")
                return
            }

            val rustTargetDir = System.getProperty("rust.target.dir")
                ?: error("rust.target.dir not set")
            val soPath = File(rustTargetDir, "libmeeting_assistant_ffi.so").absolutePath
            check(File(soPath).exists()) { "Native library not found: $soPath — run `cargo build -p ffi`" }
            System.setProperty("uniffi.component.meeting_assistant_ffi.libraryOverride", soPath)

            // Build AppConfig, passing model path explicitly to exercise the parameter.
            val config = AppConfig(
                modelPath = modelPath,
                dbPath = null,
                recordingsDir = null,
                promptsDir = null,
                anthropicApiKey = null,
            )

            // initCore() is blocking (loads Whisper model).
            core = initCore(config)
            println("[OK] AppCore initialized via AppConfig(modelPath=$modelPath)")

            // Start background worker and keep the handle for teardown.
            workerHandle = runBlocking { startWorker(core!!) }
            println("[OK] Worker started, isFinished=${workerHandle!!.isFinished()}")
        }

        @AfterAll
        @JvmStatic
        fun teardown() {
            workerHandle?.stop()
            core?.close()
        }
    }

    private fun requireCore(): AppCore = core ?: run {
        Assumptions.assumeTrue(false, "AppCore not initialized — model missing")
        error("unreachable")
    }

    @Test @Order(1)
    fun meetingsList() {
        val meetings = runBlocking { requireCore().meetingsList() }
        println("[OK] meetingsList() → ${meetings.size} meetings")
    }

    @Test @Order(2)
    fun templatesList() {
        val templates = runBlocking { requireCore().templatesList() }
        println("[OK] templatesList() → $templates")
        Assertions.assertTrue(templates.isNotEmpty(), "Expected at least one template")
    }

    @Test @Order(3)
    fun settingsGet() {
        val s = runBlocking { requireCore().settingsGet() }
        println("[OK] settingsGet() source=${s.recording.source}")
        Assertions.assertNotNull(s.paths)
    }

    @Test @Order(4)
    fun settingsSet() {
        val c = requireCore()
        val original = runBlocking { c.settingsGet() }

        // Change default_template and write back.
        val updated = original.copy(defaultTemplate = "smoke-test-template")
        runBlocking { c.settingsSet(updated) }

        val reloaded = runBlocking { c.settingsGet() }
        Assertions.assertEquals("smoke-test-template", reloaded.defaultTemplate)
        println("[OK] settingsSet() round-trip OK")

        // Restore original value.
        runBlocking { c.settingsSet(original) }
    }

    @Test @Order(5)
    fun diagnostics() {
        val d = runBlocking { requireCore().diagnostics() }
        println("[OK] diagnostics() os=${d.os} arch=${d.arch} cpalHost=${d.cpalHost}")
        println("     inputs=${d.inputDevices.size} outputs=${d.outputDevices.size} ffmpeg=${d.ffmpegOk}")
        Assertions.assertFalse(d.os.isEmpty())
        Assertions.assertFalse(d.arch.isEmpty())
    }

    @Test @Order(6)
    fun diagnosticsLogs() {
        val logs = runBlocking { requireCore().diagnosticsLogs() }
        println("[OK] diagnosticsLogs() → ${logs.size} entries")
    }

    @Test @Order(7)
    fun protocolLoadMissingReturnsNull() {
        val result = runBlocking { requireCore().protocolLoad("nonexistent-id-12345") }
        Assertions.assertNull(result)
        println("[OK] protocolLoad(nonexistent) = null")
    }

    @Test @Order(8)
    fun jobStatusMissingReturnsNull() {
        val result = runBlocking { requireCore().jobStatus("nonexistent-job-id") }
        Assertions.assertNull(result)
        println("[OK] jobStatus(nonexistent) = null")
    }

    @Test @Order(9)
    fun recordingStartAndStop() {
        val c = requireCore()
        val recording = runBlocking {
            c.recordingStart(name = "smoke-test", source = "mic", echoCancel = false)
        }
        println("[OK] recordingStart() id=${recording.id} path=${recording.audioPath}")
        Assertions.assertFalse(recording.id.isEmpty())

        val stopped = runBlocking { c.recordingStop(recording.id) }
        Assertions.assertEquals(recording.id, stopped.id)
        println("[OK] recordingStop() id=${stopped.id}")
    }

    @Test @Order(10)
    fun transcribeFileOnMissingPathThrows() {
        val ex = Assertions.assertThrows(Exception::class.java) {
            runBlocking { requireCore().transcribeFile("/nonexistent/audio.wav") }
        }
        println("[OK] transcribeFile(nonexistent) threw: ${ex.message}")
    }

    @Test @Order(11)
    fun workerHandleIsRunning() {
        val handle = workerHandle ?: run {
            Assumptions.assumeTrue(false, "WorkerHandle not available")
            return
        }
        Assertions.assertFalse(handle.isFinished(), "Worker should still be running")
        println("[OK] WorkerHandle.isFinished() = false (worker alive)")
    }
}
