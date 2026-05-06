use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use meeting_core::{CoreError, entities::{Segment, Transcript}, ports::Transcriber};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperTranscriber {
    ctx: Arc<WhisperContext>,
}

impl WhisperTranscriber {
    pub fn new(model_path: &Path) -> anyhow::Result<Self> {
        let path = model_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("model path is not valid UTF-8"))?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("failed to load whisper model from {path}: {e}"))?;
        Ok(Self { ctx: Arc::new(ctx) })
    }
}

#[async_trait]
impl Transcriber for WhisperTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Transcript, CoreError> {
        let ctx = Arc::clone(&self.ctx);
        let path = audio_path.to_path_buf();
        tokio::task::spawn_blocking(move || run_whisper(&ctx, &path))
            .await
            .map_err(|e| CoreError::Transcription(e.to_string()))?
    }
}

fn run_whisper(ctx: &WhisperContext, audio_path: &PathBuf) -> Result<Transcript, CoreError> {
    let samples = load_wav_as_mono_f32(audio_path)
        .map_err(|e| CoreError::Transcription(e.to_string()))?;

    let mut state = ctx.create_state()
        .map_err(|e| CoreError::Transcription(e.to_string()))?;

    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: -1.0,
    });
    params.set_language(Some("auto"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state.full(params, &samples)
        .map_err(|e| CoreError::Transcription(e.to_string()))?;

    let num_segments = state.full_n_segments()
        .map_err(|e| CoreError::Transcription(e.to_string()))?;

    let mut segments = Vec::new();
    let mut full_text = String::new();

    for i in 0..num_segments {
        let text = state.full_get_segment_text(i)
            .map_err(|e| CoreError::Transcription(e.to_string()))?;
        // whisper timestamps are in centiseconds → convert to ms
        let start_ms = state.full_get_segment_t0(i)
            .map_err(|e| CoreError::Transcription(e.to_string()))? as u64 * 10;
        let end_ms = state.full_get_segment_t1(i)
            .map_err(|e| CoreError::Transcription(e.to_string()))? as u64 * 10;

        full_text.push_str(&text);
        segments.push(Segment { start_ms, end_ms, text: text.trim().to_string() });
    }

    Ok(Transcript {
        text: full_text.trim().to_string(),
        segments,
        language: "auto".to_string(),
    })
}

const WHISPER_SAMPLE_RATE: u32 = 16000;

fn load_wav_as_mono_f32(path: &Path) -> anyhow::Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();

    let samples_i16: Vec<i16> = reader.samples::<i16>().collect::<Result<_, _>>()?;

    let mut samples_f32 = vec![0f32; samples_i16.len()];
    whisper_rs::convert_integer_to_float_audio(&samples_i16, &mut samples_f32)
        .map_err(|e| anyhow::anyhow!("audio conversion failed: {e}"))?;

    let mono = if spec.channels == 1 {
        samples_f32
    } else {
        whisper_rs::convert_stereo_to_mono_audio(&samples_f32)
            .map_err(|e| anyhow::anyhow!("stereo→mono failed: {e}"))?
    };

    if spec.sample_rate == WHISPER_SAMPLE_RATE {
        return Ok(mono);
    }

    tracing::info!(
        from = spec.sample_rate,
        to = WHISPER_SAMPLE_RATE,
        "resampling audio"
    );
    resample(mono, spec.sample_rate, WHISPER_SAMPLE_RATE)
}

fn resample(samples: Vec<f32>, from_rate: u32, to_rate: u32) -> anyhow::Result<Vec<f32>> {
    use rubato::{FftFixedIn, Resampler};

    const CHUNK: usize = 1024;

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        CHUNK,
        2,
        1,
    )?;

    let ratio = to_rate as f64 / from_rate as f64;
    let mut output = Vec::with_capacity((samples.len() as f64 * ratio) as usize + CHUNK);
    let mut pos = 0;

    while pos + CHUNK <= samples.len() {
        let waves_in = vec![samples[pos..pos + CHUNK].to_vec()];
        let waves_out = resampler.process(&waves_in, None)?;
        output.extend_from_slice(&waves_out[0]);
        pos += CHUNK;
    }

    if pos < samples.len() {
        let mut tail = samples[pos..].to_vec();
        tail.resize(CHUNK, 0.0);
        let waves_in = vec![tail];
        let waves_out = resampler.process_partial(Some(&waves_in), None)?;
        output.extend_from_slice(&waves_out[0]);
    }

    Ok(output)
}
