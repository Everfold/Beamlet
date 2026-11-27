# 抽象传输保证

- **下层依赖**：[05-routing.md](05-routing.md)
- **非目标范围**：不规定具体消息中间件、网络协议、接口形态、序列化格式或身份认证机制；具体传输方案由下游自行实现。

## 跨边界消息流

```mermaid
flowchart LR
    Ext["外部提交方"] <-->|"Submission 与 Outcome"| T["下游传输层 [协议与接口自定]"]
    T <-->|"Presence 与 Submission / Outcome"| A["Authority"]
    A <-->|"Dispatch 与 DispatchReply"| T
    T <-->|"Dispatch 与 DispatchReply / Presence"| W["Worker"]
```

## 下游传输必须满足的保证

1. **无状态透明边界**：传输层不得持有 `Operation` 或 `Worker` 租约状态，不得代做 `Target` 选路，亦不得在失败或超时后自动重发 `Dispatch` 或 `Submission`。
2. **定向单次投递与响应关联**：按 `Dispatch` 锁定的 `Pool` 与目标 `Worker` 精确投递一次，允许在途派发并发执行与乱序回复，每次派发至多关联一份响应；任何未达、拒收、失败通知、超时、断线或关联失配，均不得伪造 `Report` 或自动重试。
3. **控制隔离、断线防护与排空保留**：`Presence` 传播不得被业务处理背压阻塞；若下游采用面向连接的传输后端，断线重连后**不得**向 `Authority` 投递重连前积压在本地缓冲中的陈旧 `Presence`；`Worker` 停止接入新派发时，传输层须允许已接收进局部队列的在途 `Dispatch` 继续完成消费与回复。`Pool` 与 `RoleSession` 仅作路由关联核对，身份认证与访问控制由下游传输基础设施保障。
