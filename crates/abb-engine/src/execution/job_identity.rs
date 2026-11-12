/// Which job an event, a log record or a context belongs to. The job id is
/// absent until the job registers, so a title skipped or cancelled before it
/// starts has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobIdentity {
    pub(crate) operation_id: String,
    pub(crate) job_id: Option<String>,
    pub(crate) input_index: usize,
}

impl JobIdentity {
    pub(crate) fn registered(mut self, job_id: impl std::fmt::Display) -> Self {
        self.job_id = Some(job_id.to_string());
        self
    }
}
