# 会话身份与存活观测

- **下层依赖**：[03-roles.md](03-roles.md)
- **逻辑消息结构**：[messages.proto](proto/beamlet/v1/messages.proto)
- **非目标范围**：不涉及具体选路策略、派发投递、消息中间件或序列化格式。

## 身份与逻辑隔离

- **`RoleSession`**：由持久角色标识与单次运行会话标识组成；两者均必须非空，且不得通过比较标识大小或字典序推断会话新旧。重启实例必须分配全新的会话标识。
- **`Pool`**：大小写敏感的逻辑隔离分组。每个 `Worker` 归属唯一 `Pool`，每个 `Authority` 加入一个或多个 `Pool`；不同 `Pool` 之间的成员与公告完全隔离。

## Presence 快照与 Link 租约

每个 `Authority` 在本地按分组和 `RoleSession` 独立维护 `Worker` 存活视图，不同 `Authority` 允许存在短暂视图差异：

1. **会话序号与状态快照**：`Worker` 发布的 `Presence` 必须携带所属 `Pool`、自身 `RoleSession`、实际启用的精确能力列表、会话内严格递增的正整数序号 `sequence` 以及当前 `PresenceState`（`Accepting`、`Draining` 或 `Offline`）。首份有效快照即可完成登记；收到序号回退或同序号内容矛盾的公告必须拒绝。
2. **多路径转发不续期**：同一份 `sequence` 快照若经另一条通信路径重复到达，仅补充该路径的可达性，**不得**刷新首次到达时间，防止重复转发无端延长本地租约期限。
3. **`Link` 世代隔离**：每个通信路径独立记录收到最新公告时的 `Link` 世代与观测方本地单调时钟时间。仅当该会话处于 `Accepting`、至少存在一条 `Link` 保持连通、该 `Link` 当前世代与租约世代一致且未超出本地租约期限时，该 `Worker` 会话方可视为**可路由**；`Link` 断线或重连推进世代后，旧世代租约立即失效。

## 优雅停止与会话排空

```mermaid
sequenceDiagram
    participant S as 停止信号
    participant A as Authority
    participant W as Worker

    par Authority 排空
        S->>A: 触发停止
        A->>A: 停止新提交接入并丢弃未签发排队项
        A->>A: 等待已签发在途派发结束
    and Worker 排空
        S->>W: 触发停止
        W->>W: 发布 Draining 状态并停止接收新 Dispatch
        W->>W: 排空已入队任务并发送响应
        W->>W: 发布 Offline 状态退出当前会话
    end
```

## 形式化模型

- Quint 会话与租约模型：[session.qnt](quint/session/session.qnt)、[session_test.qnt](quint/session/session_test.qnt)
