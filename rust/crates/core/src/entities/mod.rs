mod transcript;
pub use transcript::{Segment, Transcript};

pub mod meeting;
pub use meeting::{now_unix, Meeting};

pub mod job;
pub use job::{ErrorClass, Job, JobKind, JobProgress, JobStatus, PipelineStage};

mod protocol;
pub use protocol::{Protocol, ProtocolAction, ProtocolTopic, StructuredProtocol};
