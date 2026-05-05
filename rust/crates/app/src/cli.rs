use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use clap::{Parser, Subcommand};
use meeting_core::usecases::{get_job_status, submit_transcription_job, transcribe_audio_file};
use crate::container::{Container, default_db_path, default_model_path};

#[derive(Parser)]
#[command(name = "meeting-assistant", about = "Meeting transcription and protocol generator")]
pub struct Cli {
    #[arg(long, env = "MEETING_ASSISTANT_MODEL")]
    model: Option<PathBuf>,

    #[arg(long, env = "MEETING_ASSISTANT_DB")]
    db: Option<PathBuf>,

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
        let container = Container::new_desktop(&model, &db)?;

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

            Command::Serve { port } => {
                let _worker = container.spawn_worker();

                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                let listener = tokio::net::TcpListener::bind(addr).await?;
                let actual_port = listener.local_addr()?.port();
                tracing::info!(port = actual_port, "HTTP server listening");
                println!("Server running on http://127.0.0.1:{actual_port}");

                let state = meeting_api::AppState {
                    transcriber: container.transcriber,
                    meeting_repo: container.meeting_repo,
                    job_repo: container.job_repo,
                };
                axum::serve(listener, meeting_api::create_router(state)).await?;
            }
        }
        Ok(())
    }
}
