//! 终端文本转换成结构化本地提交; 登记、选择和业务控制由 Authority 库负责.
use crate::text_reverse;
use anyhow::{Context, Result};
use beamlet_api::{AuthorityInfo, Pool, Responder, Selection, Submission, Task, TransportError};
use beamlet_authority::{Directory, LocalInbox, Request};
use tracing::{info, warn};

pub(crate) struct Input {
    local: crate::terminal::Input,
    info: AuthorityInfo,
    directory: Directory,
    pool: Pool,
    capability: String,
}
impl Input {
    pub(crate) fn new(
        info: AuthorityInfo,
        directory: Directory,
        local: crate::terminal::Input,
    ) -> Result<Self> {
        let pool = if info.pools.contains(&Pool::default()) {
            Pool::default()
        } else {
            info.pools.first().context("Authority 未配置 pool")?.clone()
        };
        Ok(Self {
            local,
            info,
            directory,
            pool,
            capability: text_reverse::CAPABILITY.into(),
        })
    }
}
impl LocalInbox for Input {
    async fn receive(&mut self) -> Result<Option<Request>, TransportError> {
        loop {
            let Some(line) = self.local.recv().await else {
                return Ok(None);
            };
            let text = line.map_err(TransportError::new)?;
            if let Some(pool) = text.strip_prefix("/pool ") {
                match pool.parse::<Pool>() {
                    Ok(pool) if self.info.pools.contains(&pool) => {
                        self.pool = pool;
                        info!(pool = %self.pool, "已选择 pool");
                    }
                    _ => warn!("该 pool 不属于当前 Authority"),
                }
            } else if let Some(capability) = text.strip_prefix("/capability ") {
                self.capability = capability.to_owned();
                info!(capability = %self.capability, "已选择能力");
            } else if text == "/workers" {
                info!(workers = ?self.directory.workers(&self.pool, None), "Worker 列表");
            } else {
                return Ok(Some(Request {
                    submission: Submission {
                        pool: self.pool.clone(),
                        worker: Selection::Any,
                        task: Task {
                            capability: self.capability.clone(),
                            payload: text,
                        },
                    },
                    reply: Responder::new(|outcome| async move {
                        tracing::info!(event = "response.local", response = ?outcome, "本地结果");
                        Ok(())
                    }),
                }));
            }
        }
    }
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.local.close();
        while self.local.try_recv().is_ok() {}
        Ok(())
    }
}
