# Authority 与 Worker 协作基础

- **下层依赖**：[01-kernel.md](01-kernel.md)、[02-ledger.md](02-ledger.md)
- **非目标范围**：不涉及多实例寻址分组、成员公告、会话租约、外部接入协议或具体传输机制。

## 职责分离与最简交互

```mermaid
sequenceDiagram
    participant A as Authority (持有 Ledger)
    participant W as Worker (无台账执行者)

    A->>A: "登记 Operation 并签发唯一 Attempt"
    A->>W: "交付既有 Attempt 与业务请求"
    alt 执行正常结束并返回有效证据
        W-->>A: "返回完整 Report"
        A->>A: "校验关联并推进至 Ready (决定重试或关闭)"
    else 未能取得有效 Report
        W-->>A: "未送达 / 拒收 / 执行失败通知 / 超时或失配"
        A->>A: "保持 AwaitingReport, 不重试、不伪造 Report、不关闭"
    end
```

- **`Authority`**：持有私有 `Ledger`，唯一负责登记 `Operation`、签发新 `Attempt`、向目标发起单次交付、校验并接受 `Report` 以及决定重试或关闭。不同 `Authority` 之间不共享 `Ledger`。
- **`Worker`**：无台账执行者；接收既有 `Attempt` 与业务请求并执行处理，正常结束时产出绑定原 `Operation` 与 `Attempt` 的 `Report`，执行异常未完成时返回失败通知而不伪造 `Report`。`Worker` 不持有或修改 `Ledger`，亦不得擅自再签发新 `Attempt`。共用同一个 `Worker` 不代表共享或接管任何 `Authority` 的 `Operation`。

## 二分结果契约

```mermaid
flowchart TD
    Mint["Authority 签发唯一 Attempt 并发起单次交付"] --> Res{"交付结果"}
    Res -->|"收到有效且匹配 Report"| Accept["接受 Report 回到 Ready (由上层决定重试或关闭)"]
    Res -->|"未取得有效 Report (未达 / 拒收 / 失败 / 超时 / 失配)"| Keep["保持 AwaitingReport (不重试, 不伪造, 不关闭)"]
```

- **二分裁决语义**：签发 `Attempt` 并发起单次交付后，协调层只有两种结果语义：要么收到关联匹配的真实 `Report` 并推进 `Operation` 回到 `Ready`，要么未能取得有效 `Report`（无论未达、执行前拒收、执行期异常、超时、断线还是报告失配）并完整保留 `AwaitingReport`。
- **一次性响应相关性**：每次调用至多关联一份响应；局部发送成功仅表示底层接收了回复数据，不代表接收端已推进状态机。
- **超时非取消与验收解耦**：等待期限耗尽或取消等待仅结束本地等待，不取消或回滚 `Worker` 侧已开始的执行。收到迟到 `Report` 时仅核对其非空身份与原 `Attempt` 及锁定目标匹配，不要求目标在验收瞬间仍在线。

## 关键不变量

- **未获真实证据不解除等待**：未收到有效匹配 `Report` 时（包括失败通知、超时、断线或错配 `Report`），原 `Operation` 必须保持 `AwaitingReport`，不得自动重试或无证关闭。
- **合法验收必有真实来源**：能够解除 `AwaitingReport` 的 `Report` 必须匹配当初签发并投递给目标 `Worker` 的 `(Operation, Attempt)`。
- **授权唯一不重发**：每次交付前签发的 `Attempt` 全局唯一；单次交付未取得有效 `Report` 后不得换目标重发同一次尝试。

## 形式化模型

- Quint 交互模型与测试：[roles.qnt](quint/roles/roles.qnt)、[roles_test.qnt](quint/roles/roles_test.qnt)
