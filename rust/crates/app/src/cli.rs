use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use clap::{Parser, Subcommand};
use meeting_core::usecases::transcribe_audio_file;
use crate::container::{Container, default_model_path};

#[derive(Parser)]
#[command(name = "meeting-assistant", about = "Meeting transcription and protocol generator")]
pub struct Cli {
    /// Path to a ggml whisper model file.
    /// Defaults to ~/.local/share/meeting-assistant/models/ggml-base.bin
    #[arg(long, env = "MEETING_ASSISTANT_MODEL")]
    model: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribe an audio file and print the result to stdout.
    Transcribe {
        #[arg(value_name = "FILE")]
        path: PathBuf,
    },
    /// Start the HTTP server.
    Serve {
        /// Port to listen on. Defaults to a random free port.
        #[arg(long, default_value = "0")]
        port: u16,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        let model = self.model.unwrap_or_else(default_model_path);
        let container = Container::new_desktop(&model)?;

        match self.command {
            Command::Transcribe { path } => {
                let transcript = transcribe_audio_file(
                    Arc::clone(&container.transcriber),
                    &path,
                )
                .await?;
                println!("{}", transcript.text);
            }
            Command::Serve { port } => {
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                let listener = tokio::net::TcpListener::bind(addr).await?;
                let actual_port = listener.local_addr()?.port();
                tracing::info!(port = actual_port, "HTTP server listening");
                println!("Server running on http://127.0.0.1:{actual_port}");

                let state = meeting_api::AppState {
                    transcriber: container.transcriber,
                };
                axum::serve(listener, meeting_api::create_router(state)).await?;
            }
        }
        Ok(())
    }
}
