//! 终端文本转换成结构化本地提交; 登记, 选择和业务控制由 Authority 库负责.
use crate::text_reverse;
use anyhow::{Context, Result};
use beamlet_api::{AuthorityInfo, Pool, Responder, Selection, Submission, Task, TransportError};
use beamlet_authority::{Directory, LocalInbox, Request};
use tracing::{info, warn};

/// 把终端文本转换为 Authority 本地提交的适配器, 并保存该角色的 pool 与能力选择.
pub(crate) struct Input {
    /// 终端路由到该 Authority 的文本输入队列.
    local: crate::terminal::Input,
    /// 当前 Authority 的角色会话, 标签和已加入 pools.
    info: AuthorityInfo,
    /// 与当前 Authority 登记表共享状态的只读 Worker 查询句柄.
    directory: Directory,
    /// 该 Authority 本地输入当前选定的 pool.
    pool: Pool,
    /// 该 Authority 本地文本提交当前选定的能力.
    capability: String,
}
impl Input {
    /// 把终端入口装配为指定 Authority 的本地输入.
    ///
    /// # 参数
    ///
    /// - `info`: Authority 的角色/会话身份, 标签及其加入的 pools.
    /// - `directory`: 当前 Authority 的只读 Worker 查询句柄.
    /// - `local`: 终端路由到该 Authority 的文本行接收端.
    ///
    /// # 返回值
    ///
    /// 成功返回使用默认 pool 或首个已配置 pool 的输入; Authority 未配置 pool 时返回错误.
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
    /// 处理本地选择命令, 直到收到需要提交的文本或输入结束.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Input` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 有效文本返回 `Ok(Some(Request))`, EOF 返回 `Ok(None)`; 读取失败时返回错误.
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
    /// 关闭本地输入并丢弃尚未签发的排队文本.
    ///
    /// # 参数
    ///
    /// - `self`: 当前 `Input` 的可变引用.
    ///
    /// # 返回值
    ///
    /// 返回 `Ok(())`; 不把剩余输入转换为任务.
    async fn stop(&mut self) -> Result<(), TransportError> {
        self.local.close();
        while self.local.try_recv().is_ok() {}
        Ok(())
    }
}
