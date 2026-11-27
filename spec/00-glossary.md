# 领域词汇表

- **前置约定**：[00-conventions.md](00-conventions.md)

仅收录真实存在于 Beamlet 架构与代码边界中的领域术语；术语按自底向上模块归位，模块内按概念角色细分。

---

## 原语与内核

### 原语与内核：三大核心原语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Operation` | 携带固定身份 `id`、不可变原始 `request` 与三态生命周期的状态机对象 | `request` 相同而 `id` 不同属于两个独立 `Operation`；不承担网络或存储职责 |
| `Attempt` | 针对某个指定 `Operation` 的单次执行授权，携带自身 `id` 与 `operation_id` | 重复传递同一份 `Attempt` 数据不产生新授权；自身不证明授权真实性 |
| `Report` | 一次 `Attempt` 执行结束后的结果证据，携带自身 `id`、`Attempt` 引用与 `result` | 即使 `result` 表达业务失败，对应已结束 `Attempt` 即构成合法 `Report` |

### 原语与内核：状态与错误

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `OperationStatus` | `Operation` 当前生命周期阶段的只读标签：`Ready`、`AwaitingReport`、`Closed` | 三态互斥完备，待处理 `Attempt` 身份需单独读取 |
| `Ready` | 尚未关闭且当前没有等待 `Report` 的 `Attempt` 之状态 | 既可以是新建初态，也可以是接受 `Report` 后的状态；区别于上层 `PresenceState::Ready` |
| `AwaitingReport` | 正在唯一等待指定 `attempt_id` 对应 `Report` 的状态 | 同时至多等待一个 `Attempt`，期间禁止再签发或关闭 |
| `Closed` | `Operation` 已显式终结的状态 | 允许幂等重复 `close`，永不回退；接受 `Report` 不会自动进入 `Closed` |
| `AcceptReportError` | `accept_report` 校验失败时保留原等待中 `Operation` 的错误：`OperationMismatch`、`AttemptMismatch` | 调用 `into_operation` 可无损取回原 `Operation` 继续等待 |
| `RestoreOperationError` | `restore_awaiting_report` 发现 `Attempt` 所属身份失配时的错误 | 调用 `into_parts` 可无损取回传入身份、请求与授权 |

---

## 执行台账

### 执行台账：主体与容器

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Ledger` | 在内存中管理多个 `Operation` 当前状态并记录全局已签发 `Attempt` 身份的执行台账 | 保障跨重试与跨 `Operation` 的 `Attempt` 身份不重复；不含网络与持久化 |
| `OperationView` | `Ledger` 对外提供的单个 `Operation` 只读借用视图 | 不暴露内部可变状态对象 |

### 执行台账：错误

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `LedgerError` | `Ledger` 更新拒绝原因：`OperationExists`、`OperationNotFound`、`AttemptPending`、`NoPendingAttempt`、`OperationClosed`、`AttemptIdReused`、`ReportMismatch` | 任何失败均保留原状态，且不把被拒的未使用 `Attempt` 身份记为已用 |
| `CreateOperationError` | `create_operation` 遇到重复身份时的错误 | 保留非 `Clone` 请求与身份，不覆盖既有条目 |

---

## 角色与基础交付

### 角色与基础交付：主体与容器

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Authority` | 持有 `Ledger`、负责创建 `Operation`、签发 `Attempt`、交付调用并验收 `Report` 的协调角色 | 不同 `Authority` 之间不共享 `Ledger` |
| `Worker` | 接收既有 `Attempt`、执行任务并产出 `Report` 或失败通知的无台账执行角色 | 不持有 `Ledger`，不得擅自再签发新授权；共用 `Worker` 不代表共享 `Operation` |

### 角色与基础交付：消息与入口

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `WorkerResponse` | `Worker` 对单次已接收调用的一次性响应：`Report` 或 `Failed` | 执行期 `Failed` 无法排除副作用，绝不伪造为 `Report` 或未交付 |
| `Responder` | 隐藏传输地址、消费所有权并异步完成唯一一次回复发送的回复入口 | 发送成功仅表示传输层接收回复，不证明接收端已推进状态机 |

### 角色与基础交付：结果与错误

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `DeliveryError` | 单次交付的可确认程度：`NoReceiver`、`NotDelivered`、`Unknown` | 不改变 `Operation` 既有等待状态；`Unknown` 禁止自动重试 |
| `ReplyError` | `Responder` 发送唯一回复失败时包装底层原因的错误 | 保留原始错误链，不自动重试 |

---

## 会话路由与编排

### 会话路由与编排：身份与寻址

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `RoleInstance` | 持久配置的 `role_id` 与单次运行生成的 `session_id` 组成的角色实例标识 | 禁止按 UUID 大小推断会话新旧 |
| `Pool` | 大小写敏感、可用作单段路由名的逻辑隔离分组（`1..=64` 字节 ASCII 字母、数字、`-`、` _`） | 限定 `Presence` 可见域与选路范围，不修剪空白 |
| `CapabilityId` | 按字节精确匹配的带版本能力标识（`1..=256` 字节非空字符串） | 同时约束公告声明与调用准入，无隐式版本兼容 |
| `WorkerBinding` | 绑定到 `Worker` 实例的 `pool`、`RoleInstance` 与启用 `CapabilityId` 集合 | 不包含选人策略 |
| `Selection` | 目标选定约束：`Any`（从同池匹配能力候选中任选其一）或 `Exact(RoleInstance)` | 每次提交至多选人一次；`Exact` 失效直接拒绝，绝不回退 |

### 会话路由与编排：载体与消息

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Presence` | `Worker` 发布的完整状态快照，含 `pool`、`instance`、`label`、`capabilities`、`sequence`、`state` | 会话内 `sequence` 递增；相同快照重复转发不延长租约时间 |
| `AuthorityInfo` | `Authority` 对外发现回复携带的 `RoleInstance`、`label` 与已加入 `pools` | 仅用于发现与目标选择 |
| `Task` | 由精确匹配的 `capability` 与泛型业务输入 `payload` 构成的不可变任务内容 | 作为创建 `Operation` 时的原始请求 |
| `Submission` | 外部提交创建请求的描述，含 `pool`、`Selection` 与 `Task` | 仅表达提交意图，尚未创建 `Operation` 或 `Attempt` |
| `Command` | `Authority` 请求入口命令：`Workers`（查询列表）与 `Submit`（提交执行） | `request_id` 仅作请求/回复关联，不提供创建幂等 |
| `Reply` | `Authority` 对 `Command` 的关联回复，含 `request_id`、`authority` 与 `Outcome` | 按完成顺序独立返回 |
| `Call` | 发往选定 `Worker` 的调用数据，含 `pool`、`authority`、`worker`、`attempt`、`capability`、`payload` | 只携带既有 `Attempt` 授权 |
| `Incoming` | 传输层交由 `Worker` 处理的单次接收单元，由 `Call` 与一次性 `Responder` 组成 | 回复入口专属于本次调用 |

### 会话路由与编排：主体、状态与观测

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Client` | 驱动外部发现、`Workers` 查询与 `Submit` 请求并校关联回复的客户端组件 | 不保存可变选择快照，请求只交付传输一次 |
| `PresenceState` | `Worker` 自报接收状态：`Ready`、`Draining`、`Offline` | 失联标记由观察方按本地单调时钟另行判定；`Ready` 不预留并发槽位 |
| `Connection` | 传输端点暴露的本地连接世代 `epoch` 与布尔状态 `connected`（只读视图为 `ConnectionView`） | 每次断线或重连推进 `epoch` 后，绑定于旧 `epoch` 的租约立即失效 |
| `WorkerView` | `Authority` 对某个 `Worker` 会话的本地观测视图，含 `presence`、`last_seen_ms`、`stale` | `last_seen_ms` 与 `stale` 按 `Authority` 本地单调时钟即时计算 |

### 会话路由与编排：结果与错误

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Outcome` | `Authority` 对外返回的四态结果：`Rejected`、`Workers`、`Completed`、`Unresolved` | `Rejected` 表示未创建 `Operation`；`Unresolved` 表示已签发但未取得有效 `Report` |
| `TransportError` | 传输入口或底层连接失败的包装错误 | 保留底层原因链，不自动重试 |

---

## 线协议、传输与宿主

### 线协议、传输与宿主：v3 线协议消息（`beamlet-protocol`）

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Envelope` | 携带 `version = 3` 与具体 `body` 的外层 JSON 封装 | 严格拒绝未知字段与非 `3` 版本报文 |
| `Discovery` / `DiscoveryReply` | v3 发现查询与包含 `AuthorityInfo` 的关联回复 | 允许在发现窗口内收集多份回复 |
| `WorkerCall` / `ReportData` | v3 文本协议上展开字段后的调用与报告载体 | 仅在编码层把 `Attempt`/`Report` 展开并与 `String` 文本互转 |
| `WorkerReply` | v3 线格式的 `Worker` 三态回复：`Report`、`Rejected`、`Failed` | `Rejected` 在传输契约侧映射为 `DeliveryError::NotDelivered`，`Failed` 映射为 `Unknown` |

### 线协议、传输与宿主：传输与宿主容器

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Nats` | 基于 NATS 实现跨进程通信并携带连接 `epoch` 观测的传输适配器 | `Subject` 与 `RoleInstance` 仅作路由关联，不提供安全认证 |
| `Local` | 创建隔离进程内有界路由空间、按值透传任意 Rust 类型的传输适配器 | 无需序列化与 `serde` |
| `Host` | 统一启动并发长期任务组、广播停止并限时排空的监督容器 | 不解析领域身份、配置文件或系统信号源 |
| `Shutdown` | `Host` 向受管任务广播的不可撤回停止通知 | 支持克隆与晚等待者；首次触发锁定全组排空截止时间 |
