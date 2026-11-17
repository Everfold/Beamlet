use anyhow::Result;
use beamlet_api::{Outcome, Responder, Submission};
use std::future::Future;

/// 入口交出已解析的任务与路由, 不创建领域身份.
pub(crate) struct Request {
    pub submission: Submission,
    pub reply: Reply,
}
pub(crate) trait Ingress: Send {
    fn receive(&mut self) -> impl Future<Output = Result<Option<Request>>> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<()>> + Send;
    fn flush(&mut self) -> impl Future<Output = Result<()>> + Send;
}

pub(crate) type Reply = Responder<Outcome>;
