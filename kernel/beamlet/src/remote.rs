//! 远端提交的有界交付与只读查询, 编码和连接由应用显式提供

use crate::{
    OperationObservation, ReportObservation,
    id::{AttemptId, OperationId, ReportId, RoleInstanceId, SubmissionKey},
    query,
};
use std::{error::Error as StdError, future::Future, time::Duration};
use thiserror::Error;
use tokio::time::{Instant, sleep_until, timeout_at};

// 沿用原型的单次提交预算; 接纳, 无关消息和重投均不刷新总期限.
const MAX_SENDS: usize = 3;
const SEND_TIMEOUT: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(10);

/// 提交或查询使用的专用回复通道; 单个通道不得由多个 Client 竞争消费
///
/// 方法取消后不得遗留后台重投. recv 须可取消; None 表示已丢弃一条非法消息,
/// 连接关闭须返回错误. 具体连接, 订阅和字节上限由实现与应用管理.
pub trait Transport: Send + Sync {
    /// 连接或交付错误, 不等于任务执行失败
    type Error: StdError + Send + Sync + 'static;

    /// 等待发送条件并发送原字节; 成功不代表业务接纳
    fn send(&self, bytes: &[u8]) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// 接收一条有界回复; 过滤非法传输消息时返回 None
    fn recv(&self) -> impl Future<Output = Result<Option<Vec<u8>>, Self::Error>> + Send;
}

/// 提交协议的编码边界, 不选择连接或实现重投
pub trait Codec: Send + Sync {
    /// 调用方的完整任务定义
    type Task: ?Sized + Sync;
    /// 业务输出
    type Output: Send;
    /// 业务失败证据
    type Failure: Send;
    /// 本地编码或远端解码错误
    type Error: StdError + Send + Sync + 'static;

    /// 校验并编码一次提交, 返回值在本次重投中保持不变
    ///
    /// # 错误
    /// 输入无法编码或超出协议上限时返回错误, 不发送请求
    fn encode(&self, key: SubmissionKey, task: &Self::Task) -> Result<Vec<u8>, Self::Error>;

    /// 严格解码完整回复; Client 还会校验执行关联与证据
    ///
    /// # 错误
    /// 结构, 版本或协议内容非法时返回错误, 不确认接纳
    fn decode(&self, bytes: &[u8]) -> Result<Reply<Self::Output, Self::Failure>, Self::Error>;
}

/// Authority 确认的原接纳映射; 接纳回执尚不包含 Worker 身份
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// 接纳请求的运行实例, 不是跨重启的稳定 AuthorityId
    pub authority_instance_id: RoleInstanceId,
    /// 原请求身份
    pub operation_id: OperationId,
    /// 原执行授权身份
    pub attempt_id: AttemptId,
}

/// Authority 在接纳后返回的观察, 与传输错误分开
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Outcome<O, E> {
    /// Authority 当前仍没有可返回的证据, 不表示执行结束
    Pending,
    /// 已收录证据, 其中的失败或拒绝仍是业务结果
    Report(ReportObservation<O, E>),
    /// 已有关联请求的提交或交付错误, 不合成失败 Report
    SubmissionError(String),
}

/// Codec 解码后的接纳或结果消息, 不包含临时回复地址
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Reply<O, E> {
    /// 接纳映射可以随结果一起到达
    Accepted {
        /// 原提交键
        key: SubmissionKey,
        /// 原接纳映射
        binding: Binding,
        /// None 是独立接纳回执, 不表示完成
        outcome: Option<Outcome<O, E>>,
    },
    /// Authority 返回提交错误, 不包含业务执行身份
    Rejected {
        /// 原提交键
        key: SubmissionKey,
        /// 返回错误的 Authority 实例
        authority_instance_id: RoleInstanceId,
        /// 拒绝原因, 不构成执行证据
        reason: String,
    },
}

/// 一次有界提交已取得的关联, 超时或断线不会抹去已确认的映射
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Progress {
    /// 调用方须保留此键以找回执行
    pub submission_key: SubmissionKey,
    /// 已知接纳映射; None 也不能证明未接纳
    pub binding: Option<Binding>,
    /// 已启动的发送次数, 匹配失败和发送超时也消耗次数
    pub sends: usize,
}

/// 已取得接纳映射及 Authority 返回的结果观察
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Submission<O, E> {
    /// 原提交键
    pub submission_key: SubmissionKey,
    /// 原接纳映射
    pub binding: Binding,
    /// 结果观察, Pending 和 SubmissionError 不等于业务完成
    pub outcome: Outcome<O, E>,
}

/// 提交调用错误; 不把超时, 连接错误或接纳拒绝变成业务失败
#[derive(Debug, Error)]
pub enum ClientError<C, T> {
    /// 本地编码失败, 尚未发送
    #[error("提交编码失败: {0}")]
    Encode(#[source] C),
    /// 接收连接失败, 保留原键及已知接纳映射
    #[error("提交回复接收失败: {source}")]
    Transport {
        /// 失败前已经取得的事实
        progress: Progress,
        /// 传输错误
        #[source]
        source: T,
    },
    /// 总观察期限已到, 不证明未接纳或未执行
    #[error("等待结果超时; 原提交可能已接纳或执行, 须使用原 SubmissionKey 找回")]
    TimedOut {
        /// 超时前已经取得的事实
        progress: Progress,
    },
    /// 匹配的 Authority 拒绝提交, 不含执行 Report
    #[error("提交被 Authority 拒绝: {reason}")]
    Rejected {
        /// 原提交键
        submission_key: SubmissionKey,
        /// 返回拒绝的运行实例
        authority_instance_id: RoleInstanceId,
        /// 远端原因
        reason: String,
    },
}

/// 在显式通道上顺序提交或查询, 复用连接和回复订阅
///
/// 每次调用只编码一次, 最多发送三次原字节; 每次匹配及发送共用 500ms,
/// 发送结束后间隔 1s, 总观察期限为 10s. 合法接纳或结果停止重投.
/// 创建 Client 不建立连接或启动后台任务. 丢弃等待 future 不取消远端执行,
/// 调用方仍须保留原键; 再次调用 submit 是用户显式发起的新观察预算.
/// 查询需要明确的 expected_authority 及实现 query::Codec 的编码器.
/// 当前只返回本次调用的第一份匹配观察, 订阅合并和持久恢复另行提供.
#[derive(Debug)]
pub struct Client<T, C> {
    transport: T,
    codec: C,
    expected_authority: Option<RoleInstanceId>,
}

impl<T, C> Client<T, C> {
    /// 绑定专用回复通道及 Codec; None 仅允许提交, 在该次提交中以首份合法接纳确定实例
    pub fn new(transport: T, codec: C, expected_authority: Option<RoleInstanceId>) -> Self {
        Self {
            transport,
            codec,
            expected_authority,
        }
    }
}

impl<T: Transport, C: Codec> Client<T, C> {
    /// 使用原提交键发起一次有界提交并返回结构化观察
    ///
    /// 无关, 错绑定或非法消息不确认接纳, 不刷新预算. 已确认映射不被后来的
    /// 无映射拒绝推翻. 发送失败消耗本次发送机会, 仍在总期限内接收可能迟到的结果.
    ///
    /// # 错误
    /// 编码失败, 接收连接失败, 匹配的提交拒绝或总期限到达时返回错误.
    pub async fn submit(
        &mut self,
        key: SubmissionKey,
        task: &C::Task,
    ) -> Result<Submission<C::Output, C::Failure>, ClientError<C::Error, T::Error>> {
        let bytes = self.codec.encode(key, task).map_err(ClientError::Encode)?;
        let mut progress = Progress {
            submission_key: key,
            binding: None,
            sends: 0,
        };
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        let mut next = Instant::now();
        loop {
            tokio::select! {
                biased;
                _ = sleep_until(deadline) => return Err(ClientError::TimedOut { progress }),
                // 首次发送立即就绪, 避免定时器精度使上一轮迟到回执先被消费并阻止本轮提交.
                _ = async { if progress.sends > 0 { sleep_until(next).await; } }, if progress.binding.is_none() && progress.sends < MAX_SENDS => {
                    progress.sends += 1;
                    let _sent = timeout_at(deadline.min(Instant::now() + SEND_TIMEOUT), self.transport.send(&bytes)).await;
                    next = Instant::now() + RETRY_DELAY;
                }
                received = self.transport.recv() => {
                    let Some(bytes) = received.map_err(|source| ClientError::Transport { progress, source })? else { continue };
                    let Ok(reply) = self.codec.decode(&bytes) else { continue };
                    match reply {
                        Reply::Rejected { key: received_key, authority_instance_id, reason } => {
                            if received_key == key && progress.binding.is_none()
                                && self.expected_authority.is_none_or(|id| id == authority_instance_id) {
                                return Err(ClientError::Rejected { submission_key: key, authority_instance_id, reason });
                            }
                        }
                        Reply::Accepted { key: received_key, binding, outcome } => {
                            if received_key != key
                                || self.expected_authority.is_some_and(|id| id != binding.authority_instance_id)
                                || progress.binding.is_some_and(|original| original != binding)
                                || !valid_outcome(binding, outcome.as_ref()) {
                                continue;
                            }
                            progress.binding = Some(binding);
                            if let Some(outcome) = outcome {
                                return Ok(Submission { submission_key: key, binding, outcome });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn valid_outcome<O, E>(binding: Binding, outcome: Option<&Outcome<O, E>>) -> bool {
    let Some(Outcome::Report(observation)) = outcome else {
        return true;
    };
    observation.authority_instance_id == binding.authority_instance_id
        && observation.report.operation_id == binding.operation_id
        && observation.report.attempt_id == binding.attempt_id
        && observation.report.validate().is_ok()
}

impl<T: Transport, C: query::Codec> Client<T, C> {
    /// 在已绑定 Authority 上执行一次只读查询; request_id 须由应用为每次调用独立生成
    ///
    /// 共用已有连接, 总期限 10s, 只发送一次查询. 错关联, 旧回复和非法快照不满足查询,
    /// 不延长期限. 本方法不调用提交接口, 不生成执行 ID 或将超时解释为 NotFound.
    ///
    /// # 错误
    /// 未指定 Authority, 编码失败, 连接不可用, 等待超时或匹配的 NotFound 时返回错误
    pub async fn query(
        &mut self,
        request_id: query::Id,
        selection: query::Selection,
    ) -> Result<
        query::Observation<C::Task, C::Output, C::Failure>,
        query::ClientError<C::Error, T::Error>,
    > {
        let request = query::Request {
            id: request_id,
            authority_instance_id: self
                .expected_authority
                .ok_or(query::ClientError::AuthorityRequired)?,
            selection,
        };
        let bytes = self
            .codec
            .encode_query(request)
            .map_err(query::ClientError::Encode)?;
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        timeout_at(deadline, self.transport.send(&bytes))
            .await
            .map_err(|_| query::ClientError::TimedOut { request })?
            .map_err(|source| query::ClientError::Transport { request, source })?;
        loop {
            tokio::select! {
                biased;
                _ = sleep_until(deadline) => return Err(query::ClientError::TimedOut { request }),
                received = self.transport.recv() => {
                    let Some(bytes) = received.map_err(|source| query::ClientError::Transport { request, source })? else { continue };
                    let Ok(reply) = self.codec.decode_query(&bytes) else { continue };
                    if !reply.matches(request) { continue; }
                    return match reply.observation {
                        query::Observation::NotFound => Err(query::ClientError::NotFound { request }),
                        observation => Ok(observation),
                    };
                }
            }
        }
    }

    /// 按原提交键读取执行快照, 不要求重送任务定义
    /// # 错误
    /// 与 query 相同; 原键不存在时为 NotFound
    pub async fn query_submission(
        &mut self,
        request_id: query::Id,
        key: SubmissionKey,
    ) -> Result<OperationObservation<C::Task>, query::ClientError<C::Error, T::Error>> {
        let query::Observation::Operation(view) = self
            .query(request_id, query::Selection::Submission(key))
            .await?
        else {
            unreachable!("已验证的原键查询只能返回 Operation");
        };
        Ok(view)
    }

    /// 按 OperationId 读取当前快照
    /// # 错误
    /// 与 query 相同; 原 Operation 不存在时为 NotFound
    pub async fn query_operation(
        &mut self,
        request_id: query::Id,
        id: OperationId,
    ) -> Result<OperationObservation<C::Task>, query::ClientError<C::Error, T::Error>> {
        let query::Observation::Operation(view) = self
            .query(request_id, query::Selection::Operation(id))
            .await?
        else {
            unreachable!("已验证的请求查询只能返回 Operation");
        };
        Ok(view)
    }

    /// 按 ReportId 读取完整证据, 也可读取未被选为代表结果的 Report
    /// # 错误
    /// 与 query 相同; 原 Report 不存在时为 NotFound
    pub async fn query_report(
        &mut self,
        request_id: query::Id,
        id: ReportId,
    ) -> Result<ReportObservation<C::Output, C::Failure>, query::ClientError<C::Error, T::Error>>
    {
        let query::Observation::Report(view) =
            self.query(request_id, query::Selection::Report(id)).await?
        else {
            unreachable!("已验证的证据查询只能返回 Report");
        };
        Ok(view)
    }
}
