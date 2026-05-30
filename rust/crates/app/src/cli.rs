use crate::container::{
    default_db_path, default_model_path, default_prompts_dir, default_recordings_dir, Container,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use meeting_core::{
    ports::CaptureSource,
    usecases::{
        generate_protocol, get_job_status, list_meetings, start_recording, stop_recording,
        submit_transcription_job, transcribe_audio_file,
    },
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "meeting-assistant",
    about = "Meeting transcription and protocol generator"
)]
pub struct Cli {
    #[arg(long, env = "MEETING_ASSISTANT_MODEL")]
    model: Option<PathBuf>,

    #[arg(long, env = "MEETING_ASSISTANT_DB")]
    db: Option<PathBuf>,

    #[arg(long, env = "MEETING_ASSISTANT_PROMPTS")]
    prompts_dir: Option<PathBuf>,

    #[arg(long, env = "MEETING_ASSISTANT_RECORDINGS")]
    recordings_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribe an audio file and print the result to stdout (inline, no job queue).
    Transcribe {
        #[arg(value_name = "FILE")]
        path: PathBuf,
    },
    /// Submit a transcription job and print its ID.
    Submit {
        #[arg(value_name = "FILE")]
        audio_path: PathBuf,
        /// Human-readable meeting name.
        #[arg(long, default_value = "Untitled meeting")]
        name: String,
    },
    /// Print the status of a job by ID.
    Status {
        #[arg(value_name = "JOB_ID")]
        id: String,
    },
    /// Generate a meeting protocol from a transcript file (requires ANTHROPIC_API_KEY).
    Generate {
        /// Path to the transcript text file.
        #[arg(value_name = "TRANSCRIPT_FILE")]
        transcript_path: PathBuf,
        /// Template name (filename without .md extension from the prompts directory).
        #[arg(long, short = 't')]
        template: Option<String>,
        /// Meeting name to inject into the template.
        #[arg(long, short = 'n')]
        name: Option<String>,
    },
    /// Start recording. Prints the meeting ID.
    RecordStart {
        /// Human-readable meeting name.
        #[arg(long, short = 'n')]
        name: Option<String>,
        /// Audio source: mic (default), system, or mixed.
        #[arg(long, short = 's', default_value = "mic")]
        source: String,
        /// Enable acoustic echo cancellation (mixed source only, opt-in).
        #[arg(long)]
        echo_cancel: bool,
    },
    /// Stop an active recording session.
    RecordStop {
        #[arg(value_name = "MEETING_ID")]
        id: String,
    },
    /// List all recorded meetings.
    Meetings,
    /// Start the HTTP server (also runs the background worker).
    Serve {
        #[arg(long, default_value = "0")]
        port: u16,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        let model = self.model.unwrap_or_else(default_model_path);
        let db = self.db.unwrap_or_else(default_db_path);
        let prompts = self.prompts_dir.unwrap_or_else(default_prompts_dir);
        let recordings = self.recordings_dir.unwrap_or_else(default_recordings_dir);
        let container = Container::new_desktop(&model, &db, &prompts, recordings)?;

        match self.command {
            Command::Transcribe { path } => {
                let transcript =
                    transcribe_audio_file(Arc::clone(&container.transcriber), &path).await?;
                println!("{}", transcript.text);
            }

            Command::Submit { audio_path, name } => {
                let job = submit_transcription_job(
                    Arc::clone(&container.meeting_repo),
                    Arc::clone(&container.job_repo),
                    audio_path,
                    name,
                )
                .await?;
                println!("{}", job.id);
            }

            Command::Status { id } => {
                match get_job_status(Arc::clone(&container.job_repo), &id).await? {
                    Some(job) => {
                        println!("status:   {}", job.status.as_str());
                        println!("attempts: {}", job.attempts);
                        if let Some(err) = job.last_error {
                            println!("error:    {err}");
                        }
                    }
                    None => {
                        eprintln!("job not found: {id}");
                        std::process::exit(1);
                    }
                }
            }

            Command::Generate {
                transcript_path,
                template,
                name,
            } => {
                let transcript = tokio::fs::read_to_string(&transcript_path)
                    .await
                    .map_err(|e| anyhow::anyhow!("cannot read {:?}: {e}", transcript_path))?;
                let protocol = generate_protocol(
                    Arc::clone(&container.llm),
                    Arc::clone(&container.templates),
                    &transcript,
                    template.as_deref(),
                    name.as_deref(),
                )
                .await?;
                print!("{}", protocol.markdown);
            }

            Command::RecordStart {
                name,
                source,
                echo_cancel,
            } => {
                let capture_source = match source.as_str() {
                    "system" => CaptureSource::System,
                    "mixed" => CaptureSource::Mixed,
                    _ => CaptureSource::Mic,
                };
                let meeting = start_recording(
                    Arc::clone(&container.audio_capture),
                    Arc::clone(&container.meeting_repo),
                    &container.recordings_dir,
                    name,
                    capture_source,
                    echo_cancel,
                )
                .await?;
                println!("{}", meeting.id);
                eprintln!(
                    "Recording started ({source}). Press Ctrl+C or run `record-stop {}` to stop.",
                    meeting.id
                );
            }

            Command::RecordStop { id } => {
                let meeting = stop_recording(
                    Arc::clone(&container.audio_capture),
                    Arc::clone(&container.meeting_repo),
                    &id,
                )
                .await?;
                println!("Stopped. Audio saved to: {}", meeting.audio_path.display());
            }

            Command::Meetings => {
                let meetings = list_meetings(Arc::clone(&container.meeting_repo)).await?;
                if meetings.is_empty() {
                    println!("No meetings yet.");
                } else {
                    for m in &meetings {
                        let transcript_mark = if m.transcript_text.is_some() {
                            "✓"
                        } else {
                            " "
                        };
                        println!(
                            "[{transcript_mark}] {} | {} | {}",
                            m.id,
                            m.name,
                            m.audio_path.display()
                        );
                    }
                }
            }

            Command::Serve { port } => {
                let _workers = container.spawn_workers().await;

                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                let listener = tokio::net::TcpListener::bind(addr).await?;
                let actual_port = listener.local_addr()?.port();
                tracing::info!(port = actual_port, "HTTP server listening");
                println!("Server running on http://127.0.0.1:{actual_port}");

                let state = meeting_api::AppState {
                    transcriber: container.transcriber,
                    meeting_repo: container.meeting_repo,
                    job_repo: container.job_repo,
                    llm: container.llm,
                    templates: container.templates,
                    audio_capture: container.audio_capture,
                    file_store: container.file_store,
                    recordings_dir: container.recordings_dir,
                    progress: container.progress,
                    // Legacy CLI passes template names explicitly; no resolver.
                    default_template: meeting_api::no_default_template(),
                };
                axum::serve(listener, meeting_api::create_router(state)).await?;
            }
        }
        Ok(())
    }
}
