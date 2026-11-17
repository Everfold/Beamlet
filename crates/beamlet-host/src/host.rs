use crate::{BuildError, Options, Shutdown};
use std::{collections::HashMap, convert::Infallible, error::Error, fmt, future::Future, pin::Pin};
use tokio::{
    sync::watch,
    task::JoinSet,
    time::{Instant, sleep_until},
};
use tracing::Instrument;

type Cause = Box<dyn Error + Send + Sync>;
type Task = Pin<Box<dyn Future<Output = Result<(), Cause>> + Send>>;
struct Pending {
    name: String,
    task: Task,
}

/// 装配一组长期任务; add 只保存 future, run 开始轮询后才统一启动.
pub struct Host {
    options: Options,
    pending: Vec<Pending>,
    stop: watch::Sender<bool>,
}
impl fmt::Debug for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Host")
            .field("options", &self.options)
            .field(
                "tasks",
                &self
                    .pending
                    .iter()
                    .map(|task| &task.name)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}
impl Host {
    /// 校验期限并创建监督组, 无需 Tokio 执行器, 不启动任务.
    ///
    /// # Errors
    /// 排空期限为零或超出时钟表示范围时返回错误.
    pub fn new(options: Options) -> Result<Self, BuildError> {
        options.validate()?;
        let (stop, _) = watch::channel(false);
        Ok(Self {
            options,
            pending: Vec::new(),
            stop,
        })
    }
    /// 创建本组的只读停止通知, 可克隆给所有任务和输入处理.
    pub fn shutdown(&self) -> Shutdown {
        Shutdown(self.stop.subscribe())
    }
    /// 登记一个长期任务; 正常返回也必须发生在宿主请求停止之后.
    ///
    /// name 仅用于错误定位, 可传角色 UUID 或调用方命名, 不分配/校验领域身份.
    /// 任务在 run 中启动; 错误或提前结束会停止同组任务, 不自动重启.
    pub fn add<E>(
        &mut self,
        name: impl Into<String>,
        task: impl Future<Output = Result<(), E>> + Send + 'static,
    ) where
        E: Into<Cause>,
    {
        self.pending.push(Pending {
            name: name.into(),
            task: Box::pin(async move { task.await.map_err(Into::into) }),
        });
    }
    /// 启动任务并等待外部停止信号; 即使没有任务也继续等待信号.
    ///
    /// 丢弃运行 future 会通知停止并中止其异步任务; 不等待排空, 不保证中断
    /// 已经开始的阻塞计算. 需要排空时应完成 signal 并等待本方法返回.
    ///
    /// # Errors
    /// 停止信号失败、任务错误/panic/提前结束或排空超时, 汇总为 RunError.
    pub async fn run<E>(self, signal: impl Future<Output = Result<(), E>>) -> Result<(), RunError>
    where
        E: Into<Cause>,
    {
        self.run_with_background(signal, std::future::pending::<Result<(), Infallible>>())
            .await
    }
    /// 同时驱动可选的输入处理; 它正常结束后, 服务任务仍继续运行.
    ///
    /// background 不单独 spawn, 可以借用调用方数据. 它返回错误会停止组,
    /// 宿主开始停止时直接丢弃它; 它不占用排空期限. 其余取消约定同 run.
    ///
    /// # Errors
    /// 与 run 相同, 另外汇总 background 返回的错误.
    pub async fn run_with_background<S, B>(
        mut self,
        signal: impl Future<Output = Result<(), S>>,
        background: impl Future<Output = Result<(), B>>,
    ) -> Result<(), RunError>
    where
        S: Into<Cause>,
        B: Into<Cause>,
    {
        let mut tasks = JoinSet::new();
        let mut names = HashMap::new();
        for pending in self.pending.drain(..) {
            let id = tasks.spawn(pending.task.in_current_span()).id();
            names.insert(id, pending.name);
        }
        tokio::pin!(signal);
        let mut background = Some(Box::pin(background));
        let mut deadline = None;
        let mut failures = Vec::new();
        loop {
            if deadline.is_some() && tasks.is_empty() {
                return RunError::finish(failures, Vec::new());
            }
            let failure = tokio::select! {
                biased;
                _ = sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
                    tasks.abort_all();
                    // 不等待不可合作的阻塞执行, 返回仍未收取完成的任务名称.
                    let mut unfinished: Vec<_> = names.into_values().collect();
                    unfinished.sort();
                    return RunError::finish(failures, unfinished);
                }
                result = &mut signal, if deadline.is_none() => {
                    result.err().map(|source| Failure::Signal { source: source.into() })
                }
                result = tasks.join_next_with_id(), if !tasks.is_empty() => {
                    let (id, result) = match result.expect("nonempty host task set") {
                        Ok((id, result)) => (id, result),
                        Err(error) => (error.id(), Err(Box::new(error) as Cause)),
                    };
                    let name = names.remove(&id).expect("every spawned task has a name");
                    match result {
                        Ok(()) if deadline.is_some() => continue,
                        Ok(()) => Some(Failure::UnexpectedExit { name }),
                        Err(source) => Some(Failure::Task { name, source }),
                    }
                }
                result = async { background.as_mut().expect("background is present").as_mut().await }, if background.is_some() && deadline.is_none() => {
                    background = None;
                    match result {
                        Ok(()) => continue,
                        Err(source) => Some(Failure::Background { source: source.into() }),
                    }
                }
            };
            if let Some(failure) = failure {
                failures.push(failure);
            }
            if deadline.is_none() {
                deadline = Some(Instant::now() + self.options.shutdown_timeout);
                self.stop.send_replace(true);
                background = None;
                tracing::info!(event = "host.stopping", "通知所有任务停止新接收并排空");
            }
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

/// 一次具体失败; 保留调用方原始错误, 不丢弃排空期间发生的其他失败.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Failure {
    /// 命名任务返回错误, 或其 Tokio 任务 panic/被取消.
    #[error("task {name} failed: {source}")]
    Task {
        name: String,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// 长期任务在尚未请求停止时正常返回.
    #[error("task {name} exited before shutdown")]
    UnexpectedExit { name: String },
    /// 调用方的停止信号失败.
    #[error("shutdown signal failed: {source}")]
    Signal {
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// 可选后台输入处理失败.
    #[error("background input failed: {source}")]
    Background {
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
}

/// 汇总按观测顺序发生的失败以及超过期限仍未完成的任务.
#[derive(Debug)]
pub struct RunError {
    failures: Vec<Failure>,
    unfinished: Vec<String>,
}
impl RunError {
    fn finish(failures: Vec<Failure>, unfinished: Vec<String>) -> Result<(), Self> {
        if failures.is_empty() && unfinished.is_empty() {
            Ok(())
        } else {
            Err(Self {
                failures,
                unfinished,
            })
        }
    }
    /// 包括首次触发停止的失败以及排空期间后续失败.
    pub fn failures(&self) -> &[Failure] {
        &self.failures
    }
    /// 期限结束时尚未收取完成的任务名称; 超时中止不表示远端任务或阻塞计算已结束.
    pub fn unfinished(&self) -> &[String] {
        &self.unfinished
    }
}
impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, failure) in self.failures.iter().enumerate() {
            if index != 0 {
                f.write_str("; ")?;
            }
            write!(f, "{failure}")?;
        }
        if !self.unfinished.is_empty() {
            if !self.failures.is_empty() {
                f.write_str("; ")?;
            }
            write!(
                f,
                "shutdown timed out; unfinished tasks: {}",
                self.unfinished.join(", ")
            )?;
        }
        Ok(())
    }
}
impl Error for RunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.failures
            .first()
            .map(|failure| failure as &(dyn Error + 'static))
    }
}
