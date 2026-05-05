mod transcript;
pub use transcript::{Segment, Transcript};

pub mod meeting;
pub use meeting::{Meeting, now_unix};

pub mod job;
pub use job::{Job, JobKind, JobStatus};

mod protocol;
pub use protocol::Protocol;
