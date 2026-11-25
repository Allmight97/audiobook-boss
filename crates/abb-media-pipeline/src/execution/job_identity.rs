#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobIdentity {
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
