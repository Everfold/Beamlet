# 领域词汇表

- **前置约定**：[00-conventions.md](00-conventions.md)

仅收录与特定编程语言、运行时和具体传输方案无关的核心领域与协议术语，按依赖顺序自底向上归位。

---

## 原语与内核

### 原语与内核：三大核心原语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Operation` | 携带固定身份 `id`、不可变原始 `request` 与三态生命周期的最小协调对象 | `request` 相同而 `id` 不同属于两个独立 `Operation`；不承担网络或存储职责 |
| `Attempt` | 针对某个指定 `Operation` 的单次执行授权，携带自身 `id` 与所属 `operation_id` | 重复传递同一份 `Attempt` 数据不产生新授权；自身不证明授权真实性 |
| `Report` | 一次 `Attempt` 执行结束后的结果证据，携带自身 `id`、`Attempt` 引用与 `result` | 即使 `result` 表达业务失败，对应已结束 `Attempt` 即构成合法 `Report` |

### 原语与内核：生命周期状态

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Ready` | `Operation` 尚未关闭且当前没有等待 `Report` 的 `Attempt` 之状态 | 既可以是新建初态，也可以是接受 `Report` 后的状态；区别于会话层的 `PresenceState` `Ready` |
| `AwaitingReport` | `Operation` 正在唯一等待指定 `Attempt` 对应 `Report` 的状态 | 同时至多等待一个 `Attempt`，期间禁止再签发授权或关闭 |
| `Closed` | `Operation` 已显式终结的状态 | 重复关闭保持幂等，永不回退；接受 `Report` 不会自动进入 `Closed` |

---

## 执行台账

### 执行台账：状态容器

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Ledger` | 在内存中管理多个 `Operation` 当前状态并记录全局已签发 `Attempt` 身份的状态容器 | 保障跨重试与跨 `Operation` 的 `Attempt` 身份全局不重复；不含网络与存储 |

---

## 角色与基础交付

### 角色与基础交付：协调与执行角色

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Authority` | 持有 `Ledger`、负责创建 `Operation`、签发 `Attempt`、交付调用并验收 `Report` 的协调角色 | 不同 `Authority` 之间不共享 `Ledger` |
| `Worker` | 接收既有 `Attempt`、执行任务并产出 `Report` 或失败通知的无台账执行角色 | 不持有 `Ledger`，不得擅自再签发新授权；共用 `Worker` 不代表共享 `Operation` |

### 角色与基础交付：交付故障分类

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `NoReceiver` | 目标不存在或当前无对应接收者的交付故障 | 不推进也不关闭原 `AwaitingReport` 状态 |
| `NotDelivered` | 有明确证据表明调用在进入业务执行前即未交付的故障 | 不推进原 `AwaitingReport` 状态 |
| `Unknown` | 无法排除远端已接收或已产生副作用的未决故障（超时、断线、错配、执行异常） | 禁止自动重发同一次尝试，亦不得无证关闭原 `Operation` |

---

## 会话路由与编排

### 会话路由与编排：身份与寻址

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `RoleInstance` | 持久角色标识 `role_id` 与单次运行会话标识 `session_id` 的二元组合 | 禁止按标识值大小或字典序推断会话新旧 |
| `Pool` | 大小写敏感的精确逻辑隔离分组 | 限定 `Presence` 可见域与选路候选范围，不修剪或改写 |
| `Selection` | 目标选定约束：`Any`（从匹配可用候选中任选其一）或 `Exact`（锁定唯一 `RoleInstance`） | 每次提交至多选人一次；`Exact` 失效直接拒绝，绝不回退 |

### 会话路由与编排：消息载体

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Presence` | `Worker` 发布的完整状态快照，含 `pool`、`instance`、`capabilities`、`sequence`、`state` | 会话内 `sequence` 递增；相同快照经他路重复转发不延长租约时间 |
| `AuthorityInfo` | `Authority` 对外发现回复携带的 `instance` 与所属 `pools` | 仅用于发现与选定目标 |
| `Task` | 由精确匹配的能力标识与不透明业务输入 `payload` 构成的不可变任务内容 | 作为创建 `Operation` 时的原始请求 |
| `Submission` | 外部提交创建请求的描述，含 `pool`、`Selection` 与 `Task` | 仅表达提交意图，尚未创建 `Operation` 或 `Attempt` |
| `Command` | `Authority` 请求入口命令：`Workers`（查询视图）与 `Submit`（提交执行） | `request_id` 仅用于请求/回复关联，不提供创建幂等 |
| `Reply` | `Authority` 对 `Command` 的关联回复，含 `request_id`、`authority` 与 `Outcome` | 按完成顺序独立返回 |
| `Call` | 发往唯一选定 `Worker` 的调用消息，含 `pool`、`authority`、`worker`、`attempt`、`capability`、`payload` | 只携带既有 `Attempt` 授权 |
| `WorkerResponse` | `Worker` 对单次 `Call` 的一次性响应：`Report` 或执行失败通知 `failed` | 执行期失败无法排除副作用，绝不伪造为 `Report` 或 `NotDelivered` |
| `DiscoveryReply` | 与发现查询关联的回复，含 `request_id` 与 `AuthorityInfo` | 同一次发现允许收到零份或多份回复 |

### 会话路由与编排：角色、状态与观测

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Client` | 发起发现、查询与提交并校验关联回复的调用方角色 | 不保存全局可变选择，提交只投递一次 |
| `PresenceState` | `Worker` 自报接收状态：`Ready`、`Draining`、`Offline` | 失联标记由观察方按本地单调时钟另行判定；`Ready` 不预留并发槽位 |
| `Connection` | 观测方读取的底层连接视图，包含单调递增连接世代 `epoch` 与布尔状态 `connected` | 每次断线或重连推进 `epoch` 后，旧 `epoch` 租约立即失效 |
| `WorkerView` | `Authority` 对某个 `Worker` 会话的本地观测视图，含 `presence` 与 `stale` | `stale` 按 `Authority` 本地单调时钟与连接状态即时判定 |

### 会话路由与编排：结果裁决

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Outcome` | `Authority` 对外返回的四态裁决：`Rejected`、`Workers`、`Completed`、`Unresolved` | `Rejected` 承诺未创建 `Operation`；`Completed` 必对应已验收 `Report`；`Unresolved` 保持原 `AwaitingReport` |
