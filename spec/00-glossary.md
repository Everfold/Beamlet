# 领域词汇表

- **前置约定**：[00-conventions.md](00-conventions.md)

仅收录与特定编程语言、运行时和具体传输方案无关的核心领域术语，按依赖顺序自底向上归位。

---

## 原语与内核

### 原语与内核：三大核心原语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Operation` | 携带固定身份、不可变原始请求与三态生命周期的最小协调对象 | 请求内容相同而身份不同属于两个独立 `Operation`；不承担网络或存储职责 |
| `Attempt` | 针对指定 `Operation` 的单次执行授权，同时关联自身身份与所属 `Operation` 身份 | 重复传递同一份 `Attempt` 数据不产生新授权；数据自身不证明授权真实性 |
| `Report` | 一次 `Attempt` 执行结束后的结果证据，绑定自身身份、所对应 `Attempt` 引用与业务结果 | 即使业务结果代表业务失败，对应已结束 `Attempt` 仍构成合法 `Report` |

### 原语与内核：生命周期状态

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Idle` | `Operation` 尚未关闭且当前没有在途 `Attempt` 的空闲状态 | 既可以是新建初态，也可以是接受 `Report` 后的状态 |
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
| `Authority` | 持有 `Ledger`、负责创建 `Operation`、签发 `Attempt`、派发执行并验收 `Report` 的协调角色 | 不同 `Authority` 之间不共享 `Ledger` |
| `Worker` | 接收既有 `Attempt`、执行任务并产出 `Report` 或失败通知的无台账执行角色 | 不持有 `Ledger`，不得擅自再签发新授权；共用 `Worker` 不代表共享 `Operation` |

---

## 会话与存活

### 会话与存活：身份、分组与状态

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `RoleSession` | 持久角色标识与单次运行会话标识组成的角色会话身份 | 禁止按标识值大小或字典序推断会话新旧 |
| `Pool` | 大小写敏感的精确逻辑隔离分组 | 限定 `Presence` 可见域与选路候选范围，不修剪或改写 |
| `Presence` | `Worker` 发布的完整状态快照，含所属分组、会话身份、启用能力集、会话序号与当前状态 | 会话内序号单调递增；相同快照经他路转发不延长租约时间 |
| `PresenceState` | `Worker` 自报接收状态：`Accepting`（接收新派发中）、`Draining`（排空中）、`Offline`（已退出） | 失联标记由观察方按本地单调时钟另行判定；`Accepting` 不预留并发槽位 |
| `Link` | 观测方读取的逻辑通信链路状态，包含单调递增世代与当前是否连通 | 每次断线或重连推进世代后，绑定旧世代的租约立即失效 |

---

## 选路与提交

### 选路与提交：任务、派发与裁决

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| `Task` | 由精确匹配的能力标识与不透明业务输入载荷构成的不可变任务内容 | 贯穿 `Submission`、`Operation` 与 `Dispatch` 的唯一请求载体 |
| `Target` | 提交目标约束：`Any`（从匹配可用候选中任选其一）或 `Exact`（锁定唯一 `RoleSession`） | 每次提交至多选人一次；`Exact` 不可路由时直接拒绝，绝不回退 |
| `Submission` | 外部提交创建请求的描述，含目标分组、`Target` 与 `Task` | 仅表达提交意图，尚未创建 `Operation` 或 `Attempt` |
| `Dispatch` | 发往唯一选定 `Worker` 的派发消息，含目标分组、`Authority`、`Worker`、既有 `Attempt` 与 `Task` | 只传递既有 `Attempt` 授权 |
| `DispatchReply` | `Worker` 对单次 `Dispatch` 的一次性响应：完整 `Report` 或执行期失败通知 | 执行期失败无法排除副作用，绝不伪造为 `Report` 或执行前拒收 |
| `Outcome` | 对单次 `Submission` 的三态终局裁决：`Rejected`、`Completed`、`Unresolved` | `Rejected` 承诺未创建 `Operation`；`Completed` 必对应已验收 `Report`；`Unresolved` 保持原 `AwaitingReport` |
