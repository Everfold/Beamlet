# Beamlet 架构与协议规范

Beamlet 规范自底向上组织，下层不依赖上层；层次越靠下，与业务及传输的耦合度越低。

## 载体分工

- **Markdown（`spec/*.md`）**：定义核心原语、状态空间、转换契约、不变量与抽象传输保证。
- **Quint（`spec/quint/`）**：定义可执行的状态机与交互模型，并通过单元测试、随机模拟和 TLC 模型检验验证不变量。
- **Protobuf Edition 2026（`spec/proto/`）**：定义与实现语言及传输后端无关的跨角色逻辑消息结构，任意业务载荷使用 `bytes` 表示，不规定线编码格式。

## 分层依赖图谱

```mermaid
flowchart BT
    CONV["约定与词典 (00-conventions.md, 00-glossary.md)"]
    KRN["原语与内核 (01-kernel.md, quint/kernel)"]
    LDG["执行台账 (02-ledger.md, quint/ledger)"]
    ROL["角色与基础派发 (03-roles.md, quint/roles)"]
    SES["会话与存活观测 (04-session.md, quint/session, proto/beamlet/v1)"]
    RTE["目标、派发与裁决 (05-routing.md, quint/routing, proto/beamlet/v1)"]
    TRP["抽象传输保证 (06-transport.md)"]

    CONV --> KRN --> LDG --> ROL --> SES --> RTE --> TRP
```

## 文档索引

| 文件 | 形式化模型 / IDL | 核心范围 |
| --- | --- | --- |
| [00-conventions.md](00-conventions.md) | — | 规范性关键词、时钟与期限准则、术语书写规则 |
| [00-glossary.md](00-glossary.md) | — | 按依赖顺序与角色分类的领域词汇表 |
| [01-kernel.md](01-kernel.md) | [spec/quint/kernel/](quint/kernel/state_machine.qnt) | `Operation`、`Attempt`、`Report` 三种原语及内核状态机 |
| [02-ledger.md](02-ledger.md) | [spec/quint/ledger/](quint/ledger/ledger.qnt) | `Ledger` 多 `Operation` 管理与 `Attempt` 身份防重用 |
| [03-roles.md](03-roles.md) | [spec/quint/roles/](quint/roles/roles.qnt) | `Authority` 与 `Worker` 关系、二分结果契约与不变量 |
| [04-session.md](04-session.md) | [spec/quint/session/](quint/session/session.qnt)、[messages.proto](proto/beamlet/v1/messages.proto) | `RoleSession`、`Pool`、`Presence` 快照与本地租约 |
| [05-routing.md](05-routing.md) | [spec/quint/routing/](quint/routing/routing.qnt)、[messages.proto](proto/beamlet/v1/messages.proto) | `Target` 零回退、`Dispatch` 准入与三态 `Outcome` 裁决 |
| [06-transport.md](06-transport.md) | — | 下游传输实现须满足的无状态投递、控制隔离与排空保证 |
