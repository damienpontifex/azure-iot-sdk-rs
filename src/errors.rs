#[derive(Debug, Clone)]
pub enum InitilizeError {
    Connect,
    Subscribe,
    SenderTask,
    PingTask,
    ReceiverTask,
    DataRequest,
}
