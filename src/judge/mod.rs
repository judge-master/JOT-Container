pub mod checker;
pub mod compiler;
pub mod executor;
pub mod grader;

pub struct JudgeJob {
    request: jot_proto::judge::v1::JudgeRequest,
    response_channel: tokio::sync::mpsc::Sender<jot_proto::judge::v1::JudgeEvent>,
}
pub struct JudgeOperator {
    max_concurrent: u16,
    job_queue: Vec<JudgeJob>,
}

impl JudgeOperator {
    pub fn new(max_concurrent: u16) -> Self {
        Self {
            max_concurrent,
            job_queue: Vec::new(),
        }
    }

    pub fn submit(
        &mut self,
        request: jot_proto::judge::v1::JudgeRequest,
    ) -> tokio_stream::wrappers::ReceiverStream<jot_proto::judge::v1::JudgeEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(8);

        self.job_queue.push(JudgeJob {
            request,
            response_channel: tx,
        });

        tokio_stream::wrappers::ReceiverStream::new(rx)
    }
}
