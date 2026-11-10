//! 显式启动的 Tokio 驱动组, 共享停止请求与有限排空期限
//!
//! 应用提供运行时, 驱动 future 和标签, 自行创建角色及基础设施. 本模块不安装
//! 系统信号, 不发布通知, 不生成执行证据. 构造时不启动任务; 只有 `start` 启动任务.
//! 丢弃组或排空 future 会请求中止直接持有的驱动任务, 只有等待 `drain` 完成才能确认收回.
//! 驱动自行创建的工作和资源由驱动负责关闭, 不递归收回脱离本组的任务.
//! 排空期限依赖任务让出执行权, 中止 future 不证明外部效果停止.

use std::{collections::HashMap, future::Future, time::Duration};
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::watch,
    task::{AbortHandle, Id, JoinError, JoinSet},
    time::{Instant, timeout_at},
};

/// 应用可选择的默认排空期限, 不会自动启动排空
pub const DEFAULT_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn valid_drain_timeout(timeout: Duration) -> bool {
    (Duration::from_millis(1)..=Duration::from_secs(60)).contains(&timeout)
}

/// 驱动收到停止请求后的处理方式
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopPolicy {
    /// 继续轮询, 在共同期限内完成已接纳工作
    Drain,
    /// 请求停止时立即中止, 用于无需排空的附属任务
    Abort,
}

/// 只读停止信号; 不可撤销, 控制端被丢弃也视为停止
#[derive(Clone, Debug)]
pub struct Shutdown {
    receiver: watch::Receiver<bool>,
}

impl Shutdown {
    /// 是否已请求停止; 不表示角色已经处理该请求或排空
    pub fn is_draining(&self) -> bool {
        *self.receiver.borrow() || self.receiver.has_changed().is_err()
    }

    /// 等待停止请求, 重复调用立即返回; 可安全放入 `select!`
    pub async fn requested(&mut self) {
        let _ = self.receiver.wait_for(|requested| *requested).await;
    }
}

/// 停止后的正常驱动完成方式, 不表示远端收录或持久化
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    /// 驱动在停止请求之后正常返回
    Drained,
    /// 按 `StopPolicy::Abort` 中止并收回附属任务
    Aborted,
}

/// 驱动未能确认正常退出的原因, 不是业务执行失败 Report
#[derive(Debug, Error)]
pub enum Failure<E> {
    /// 应用驱动返回原始错误
    #[error("驱动失败: {0}")]
    Driver(#[source] E),
    /// 任务 panic 或非预期取消
    #[error("驱动任务异常: {0}")]
    Task(#[source] JoinError),
    /// 运行期间返回成功也不能冒充正常排空
    #[error("驱动在停止请求之前意外返回")]
    UnexpectedExit,
    /// 共同期限到达时尚未确认完成, 随后已中止并收回
    #[error("排空期限到达, 执行或交付未确认")]
    DeadlineExceeded,
}

/// 一个驱动的退出结果; 标签由调用方定义, 可对应一个组合中的多个角色
#[derive(Debug)]
pub struct Exit<K, E> {
    /// 启动时的原标签, 即使任务 panic 也保留
    pub label: K,
    /// 此驱动的独立结果
    pub result: Result<Completion, Failure<E>>,
}

/// 已收回全部所属任务之后的观察; 不承诺外部工作已停止
#[derive(Debug)]
pub struct Summary<K, E> {
    /// 按收回顺序返回, 不保证启动顺序
    pub exits: Vec<Exit<K, E>>,
    /// 是否曾因共同期限到达而中止剩余任务
    pub timed_out: bool,
}

#[derive(Debug)]
struct Entry<K> {
    label: K,
    policy: StopPolicy,
    abort: AbortHandle,
}

#[derive(Debug)]
struct Finished<E> {
    result: Result<(), E>,
    draining: bool,
}

/// 应用显式组装的驱动组, 不自动添加角色或基础设施
#[derive(Debug)]
pub struct Group<K, E> {
    runtime: Handle,
    timeout: Duration,
    stop: watch::Sender<bool>,
    tasks: JoinSet<Finished<E>>,
    entries: HashMap<Id, Entry<K>>,
    exits: Vec<Exit<K, E>>,
    deadline: Option<Instant>,
}

impl<K, E: Send + 'static> Group<K, E> {
    /// 选择已有 Tokio 运行时和 1..=60000 毫秒的共同期限, 不启动任何任务
    /// # 错误
    /// 排空期限超出允许范围时拒绝
    pub fn new(runtime: Handle, timeout: Duration) -> Result<Self, Error> {
        if !valid_drain_timeout(timeout) {
            return Err(Error::InvalidDrainTimeout);
        }
        let (stop, _) = watch::channel(false);
        Ok(Self {
            runtime,
            timeout,
            stop,
            tasks: JoinSet::new(),
            entries: HashMap::new(),
            exits: Vec::new(),
            deadline: None,
        })
    }

    /// 在指定运行时上启动一个驱动, 将只读停止信号交给应用提供的构造闭包
    ///
    /// 标签只用于关联结果, 不负责角色身份去重. 构造闭包同步执行, 返回的 future
    /// 由任务组持有. 应用应在调用前完成可失败的基础设施准备.
    /// # 错误
    /// 已请求停止时拒绝, 不调用构造闭包
    pub fn start<F, Fut>(&mut self, label: K, policy: StopPolicy, start: F) -> Result<(), Error>
    where
        F: FnOnce(Shutdown) -> Fut,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
    {
        if self.deadline.is_some() {
            return Err(Error::AlreadyStopping);
        }
        let shutdown = Shutdown {
            receiver: self.stop.subscribe(),
        };
        let observer = shutdown.clone();
        let driver = start(shutdown);
        let abort = self.tasks.spawn_on(
            async move {
                let result = driver.await;
                Finished {
                    result,
                    draining: observer.is_draining(),
                }
            },
            &self.runtime,
        );
        self.entries.insert(
            abort.id(),
            Entry {
                label,
                policy,
                abort,
            },
        );
        Ok(())
    }

    /// 仅第一次请求设置期限; 不重置预算, 不等待角色处理或排空
    pub fn stop(&mut self) {
        if self.deadline.is_none() {
            self.deadline = Some(Instant::now() + self.timeout);
            self.stop.send_replace(true);
            for entry in self.entries.values() {
                if entry.policy == StopPolicy::Abort {
                    entry.abort.abort();
                }
            }
        }
    }

    /// 等待任意驱动意外完成, 保留其结果并请求其余驱动停止
    ///
    /// 应用可与自己的信号或命令 `select!`. 取消此等待不丢失任务结果.
    /// 已请求停止时立即返回; 空组保持等待, 不推断应用应当退出.
    pub async fn wait(&mut self) {
        if self.deadline.is_some() {
            return;
        }
        if let Some(result) = self.tasks.join_next_with_id().await {
            self.record(result, false);
            self.stop();
        } else {
            std::future::pending::<()>().await;
        }
    }

    /// 请求停止并在原共同期限内排空, 到期中止剩余任务, 收回全部后返回
    ///
    /// 必须等待此 future 完成以取得收回保证. 不因一个驱动出错而提前丢弃其他驱动,
    /// 不重置 `stop` 已设置的期限; 已超期的未收回结果保守保留为未确认.
    pub async fn drain(mut self) -> Summary<K, E> {
        self.stop();
        let deadline = self.deadline.expect("stop 已设置排空期限");
        let timed_out = loop {
            if self.tasks.is_empty() {
                break false;
            }
            if Instant::now() >= deadline {
                break true;
            }
            match timeout_at(deadline, self.tasks.join_next_with_id()).await {
                Ok(Some(result)) => self.record(result, false),
                Ok(None) => break false,
                Err(_) => break true,
            }
        };
        if timed_out {
            self.tasks.abort_all();
            while let Some(result) = self.tasks.join_next_with_id().await {
                self.record(result, true);
            }
        }
        Summary {
            exits: self.exits,
            timed_out,
        }
    }

    fn record(&mut self, joined: Result<(Id, Finished<E>), JoinError>, expired: bool) {
        let id = match &joined {
            Ok((id, _)) => *id,
            Err(error) => error.id(),
        };
        let entry = self.entries.remove(&id).expect("任务必有启动时的标签");
        let result = if expired {
            Err(Failure::DeadlineExceeded)
        } else {
            match joined {
                Ok((
                    _,
                    Finished {
                        result: Err(error), ..
                    },
                )) => Err(Failure::Driver(error)),
                Ok((
                    _,
                    Finished {
                        draining: false, ..
                    },
                )) => Err(Failure::UnexpectedExit),
                Ok(_) => Ok(Completion::Drained),
                Err(error)
                    if self.deadline.is_some()
                        && entry.policy == StopPolicy::Abort
                        && error.is_cancelled() =>
                {
                    Ok(Completion::Aborted)
                }
                Err(error) => Err(Failure::Task(error)),
            }
        };
        self.exits.push(Exit {
            label: entry.label,
            result,
        });
    }
}

/// 配置或启动被拒绝; 不启动新任务
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// 期限超出当前原型的配置范围
    #[error("排空期限必须在 1..=60000 毫秒内")]
    InvalidDrainTimeout,
    /// 停止请求之后不能再增加驱动
    #[error("驱动组已请求停止, 不接受新驱动")]
    AlreadyStopping,
}
