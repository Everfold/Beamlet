# L0-A: 基础约定

## 规范性关键词

按 RFC 2119 定义强制等级：

- **必须（MUST）** / **不得（MUST NOT）**：硬性要求，违背即不合规。
- **应当（SHOULD）** / **不应当（SHOULD NOT）**：默认要求，仅在有明确理由时允许偏离。
- **可以（MAY）**：可选行为。

## 基础规则

1. **载荷不透明**：L1–L3 **不得**解析业务请求或结果内容，统一以 `OpaquePayload` 表示；L4 **可以**特化具体编码，但**不得**改变下层状态转换规则。
2. **领域身份精确相等**：`operation_id`、`attempt_id`、`report_id` 为非空不透明字符串，**必须**按字节精确相等比较，**不得**修剪空白、折叠大小写、解析前缀或按字典序推断新旧。
3. **非零 UUID**：`role_id` 与 `session_id` **不得**使用 nil UUID（`00000000-0000-0000-0000-000000000000`）。
4. **本地单调时钟**：Timeout、TTL 与排空期限**必须**基于观测方本地单调时钟计算，**不得**依赖墙上时钟或跨进程时钟同步。
5. **正数期限与容量**：有限期限与缓冲容量**必须**在构造阶段拒绝零值及越界值。
6. **逻辑 IDL 定位**：`spec/proto/`（Protobuf Edition 2026）仅定义逻辑结构与单向包依赖，不强制采用二进制 Protobuf 线格式。
7. **术语书写**：正文使用中文；领域专有名词、状态名与裁决名保留英文原词，词义以 [00-glossary.md](00-glossary.md) 为唯一准据。

## 追踪编号

- `INV-<MODULE>-<NN>`：安全不变量，对应 `spec/quint/` 同前缀 `inv_*` 谓词。
- `SEM-<MODULE>-<NN>`：并发、超时或传输绑定语义约束。

| 前缀 | 规范文件 |
| --- | --- |
| `PRM` | [01-primitives.md](01-primitives.md) |
| `KRN` | [02-kernel.md](02-kernel.md) |
| `LDG` | [03-ledger.md](03-ledger.md) |
| `REG` | [04-protocol.md](04-protocol.md)（租约格） |
| `DLV` | [04-protocol.md](04-protocol.md)（定向交付） |
| `BND` | [05-binding-nats.md](05-binding-nats.md) |
| `HST` | [06-host-and-rust.md](06-host-and-rust.md) |
