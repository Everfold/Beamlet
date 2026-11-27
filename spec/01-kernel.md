# 原语与内核状态机

- **非目标范围**：不涉及任何存储、并发、多 `Operation` 集合、历史已用 `Attempt` 记录，亦不决定重试或关闭策略。

## 三种核心原语

Beamlet 的所有协调语义建立在三种不可变原语之上，其身份与业务载荷在内核层完全不透明：

- **`Operation`**：由固定身份、不可变原始请求以及三态生命周期构成。原始请求相同而身份不同的实例视为彼此独立的 `Operation`。
- **`Attempt`**：针对唯一 `Operation` 签发的单次执行授权，同时绑定自身身份与所属 `Operation` 身份。重复传递同一份 `Attempt` 数据不产生新授权；数据自身亦不证明授权真实性。
- **`Report`**：一次 `Attempt` 执行结束后的不可变结果证据，同时绑定自身身份、所对应 `Attempt` 引用（所属 `Operation` 身份与 `Attempt` 身份）及业务结果。业务执行语义失败只要对应已结束 `Attempt`，仍构成合法 `Report`。

## 生命周期状态机

```mermaid
stateDiagram-v2
    [*] --> Ready: "创建 Operation"
    Ready --> AwaitingReport: "签发 Attempt"
    AwaitingReport --> Ready: "接受匹配 Report"
    AwaitingReport --> AwaitingReport: "拒绝不匹配 Report [保持原状态]"
    Ready --> Closed: "显式关闭"
    Closed --> Closed: "重复关闭 [幂等]"
```

## 状态转换与恢复规则

```mermaid
flowchart TD
    Act["内核动作"] --> C["签发 Attempt"]
    Act --> A["接受 Report"]
    Act --> Cl["关闭 Operation"]

    C -->|"Ready"| C1["转入 AwaitingReport 并产出 Attempt"]
    C -->|"AwaitingReport 或 Closed"| C2["拒绝 [等待中或已关闭不允许签发]"]

    A -->|"AwaitingReport"| A1{"Report 所属 Operation 是否一致 ?"}
    A1 -->|"否"| AE1["拒绝 [Operation 不匹配, 保持原等待状态]"]
    A1 -->|"是"| A2{"Report 所属 Attempt 是否为当前等待 ?"}
    A2 -->|"否"| AE2["拒绝 [Attempt 不匹配, 保持原等待状态]"]
    A2 -->|"是"| AOK["转入 Ready"]
    A -->|"Ready 或 Closed"| AE3["拒绝 [无等待中的 Attempt 或已关闭]"]

    Cl -->|"Ready 或 Closed"| Cl1["转入 Closed"]
    Cl -->|"AwaitingReport"| Cl2["拒绝 [等待报告期间禁止关闭]"]
```

- **校验顺序与无副作用拒绝**：接受 `Report` 时必须先校验当前处于 `AwaitingReport`，再依次校验所属 `Operation` 身份与当前等待的 `Attempt` 身份；任一条件不满足则拒绝并完整保留原状态及数据不变。
- **单等待与显式关闭**：同一时刻一个 `Operation` 至多等待一个 `Attempt`。接受 `Report` 成功仅回到 `Ready` 而非自动关闭；继续签发新 `Attempt` 或显式关闭由外层决定。
- **外层新身份责任**：内核不保存历史 `Attempt` 集合，外层在每次合法签发前必须提供此前未曾使用过的新 `Attempt` 身份。
- **外部已知状态恢复**：当外部拥有者已独立确认状态合法时，内核允许无副作用恢复既有状态而不重新签发授权或触发关闭：
  - **恢复 `AwaitingReport`**：校验传入 `Attempt` 所属的 `Operation` 身份与目标 `Operation` 一致；不一致则拒绝并保持无副作用，一致则恢复至对应等待态。
  - **恢复 `Closed`**：直接恢复至终态 `Closed`。

## 关键不变量

- **身份与请求不可变**：任何状态转换均不得修改 `Operation` 的固定身份与原始请求；被拒绝的动作不得改变生命周期状态。
- **`Closed` 单调不可逆**：进入 `Closed` 后不可回到 `Ready` 或 `AwaitingReport`。
- **双引用绑定**：仅当处于 `AwaitingReport` 且 `Report` 同时匹配当前 `Operation` 与当前等待中的 `Attempt` 时，状态才可回到 `Ready`。

## 形式化模型

- Quint 原语与转换模型：[primitives.qnt](quint/kernel/primitives.qnt)、[state_machine.qnt](quint/kernel/state_machine.qnt)
- Quint 模型验证与测试：[kernel.qnt](quint/kernel/kernel.qnt)、[kernel_test.qnt](quint/kernel/kernel_test.qnt)
