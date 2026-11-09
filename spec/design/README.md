# Beamlet 设计入口

状态: 2026-11-10 确认的目标设计, 为后续实现提供依据. 本目录明确区分领域契约, 实现草案与已有原型; 文档中的目标能力不代表代码已具备该保证.

## 阅读顺序

| 文档 | 回答的问题 |
| --- | --- |
| 本页 | 原语, 分层, 角色, 部署与设计取舍 |
| [Agent 实体与状态](agent-model.md) | Agent 数据, preset, Loop 状态, 输入, 单写者与 fork |
| [Operation 组合与 Agent Loop](operation-composition.md) | Loop, LLM Request, Tool, Codemode 如何关联与完成 |
| [执行 Client 与结果观察](execution-client.md) | Agent 在 Worker 上如何提交, 查询和接收 Report |
| [持久化与恢复](persistence-recovery.md) | Diesel/SQLite, 异步提交, 故障窗口与恢复限制 |
| [实现顺序与验收](implementation-plan.md) | 当前差距, 前置决策与分阶段验收 |

[术语表](../../GLOSSARY.md)定义领域用语. [Node 与角色组合](node-composition.md), [三段回执](receipts.md), [Hello Console](hello-console.md)记录已有原型. [非 Durable 协议 v0](volatile-protocol.md)保留早期里程碑, 其中旧提交接口和旧实现状态不作为新实现依据.

## 核心模型

| 概念 | 职责 |
| --- | --- |
| Operation | 有独立身份的不可变执行请求; 原始输入确定后不因重传或补充输入改变 |
| Attempt | Authority 对某个 Operation 的一次执行授权; 不等于一次消息投递 |
| Report | 关联 Operation/Attempt 的结果与证据; 保留, 采用与业务成功是不同事实 |
| Agent | 独立持久化的实体, 保存配置, 输入, 历史引用与推进状态 |
| Capability / Tool | Capability 定义可执行行为; Tool 是面向 LLM 的调用接口, 两者与 Operation 不要求一一对应 |

Agent Loop 的执行是 Operation. 每次 LLM Request 是独立 Operation, 不称为 Agent 的一次调用. Codemode 的一次代码执行和具体工具调用也可以是 Operation. Agent 实体跨 Loop 保留数据, 不把实体永久生命周期等同于一条永不结束的 Operation. Loop 的启动输入范围和完成策略必须明确, 不强制一条用户消息对应一个 Loop 或一个 Turn.

```text
Agent A
  -> Loop Operation O / Attempt O1
       -> LLM Request L1 / Report LR1
       -> Codemode C
            -> Tool T1 / Report TR1
            -> Tool T2 / Report TR2
            -> Codemode Report CR
       -> LLM Request L2 / Report LR2
       -> Loop Report OR
```

Report 当前代码中的 `Effect` 是外部影响的证据结论, 不是独立计算原语, 也不表示编程语言的 Effect 系统. `None / Observed / Unknown` 的语义继续保留. 新名称尚未定案, `EffectEvidence` 和 `ImpactStatus` 只是讨论过的候选, 本批不改 API 或编码. 执行失败不推导无影响, 执行成功不证明影响仅发生一次.

## 执行底座的状态

| 观察者 | 当前阶段 | 含义与限制 |
| --- | --- | --- |
| Authority | Accepted -> Dispatched -> Reported | 已接纳, 已记录派发, 已收录至少一份 Report; 有效 Report 先到时可直接进入 Reported |
| Worker | Queued -> Running -> Completed | 已接纳等待执行, 已进入执行, 已保存结果; 排队阶段拒绝也可直接 Completed |
| Report | Succeeded / Failed / Rejected | 执行返回输出, 执行失败或执行前拒绝; 与外部影响证据独立 |

这些是不同节点的局部事实, 不能合成一条同步全局状态机. `Dispatched` 不证明 Worker 已接纳, `Reported` 不证明业务成功. 当前 Kernel 的执行前 Rejected 必须带无外部影响结论, 不能把执行后错误伪装成执行前拒绝.

原型的 `WaitStatus::Waiting/TimedOut` 只记录本地等待观察, 不是新的领域生命周期, 也不是 Agent 的 Waiting. 超时后仍可收录 Report, 不自动取消, 改派或重新授权. Agent 的 Processing/Waiting/Suspended 与 Worker Running 可以同时成立: 父执行等待时仍未完成.

## 代码分层

```text
应用 / Console / Node / 未来 UI
                |
         beamlet-agent
                |
             beamlet
                |
         beamlet-kernel
```

| 模块 | 目标职责 |
| --- | --- |
| beamlet-kernel | 身份, 执行领域对象, 纯状态规则与证据绑定校验; 不依赖 Agent, 数据库或消息编码 |
| beamlet | 通用执行账本与驱动, Authority/Worker, 提交和观察 Interface; 不理解 LLM Loop 策略 |
| beamlet-agent | Agent 实体, preset/override, Loop 策略, 输入构造, 结果采用与 fork |
| beamlet-agent-tool | 工具及执行环境的集成位置; 不另建授权账本 |
| beamlet-durable / 消息 Adapter | 执行持久化及消息交付的实现; Agent 存储映射由 Agent 层拥有 |

这是目标职责, 不是要求每个 Interface 立刻新增 crate. 当前 `beamlet-agent::Agent` 只是 Authority/Worker 本地组合, 通用 Authority/Worker 包装也位于该 crate; 实现新 Agent 前需要调整名称与归属. 底层不得为了执行 Agent capability 而反向依赖 Agent 模块; 由应用注册或传入执行逻辑.

官方持久化以 Diesel + SQLite 为主. 用户可以提供符合契约的后端, 本阶段不设计或实现其他后端. Agent 存储与执行账本逻辑分离, 可以共用数据库与连接管理, 不要求两个数据库或全局事务.

## 角色与部署

```mermaid
flowchart LR
    UI["应用 / UI"] -->|"输入 / 控制 / 查询"| AG["Worker A: Agent Loop"]
    AG -->|"Client: 提交 / 查询 / 观察"| AU["Authority"]
    AU -->|"授权 Attempt"| W["Worker B: LLM / Tool / Codemode"]
    W -->|"Report"| AU
    AU -->|"已保留的执行观察"| AG
    AG <--> AS["Agent 存储"]
    AU <--> ES["Authority 执行账本"]
    W <--> WS["Worker 接纳 / 执行 / 交付记录"]
```

Client 是提交和观察能力, 不要求独立服务. Worker 可以同时使用 Client 能力提交子 Operation. Authority 拥有 Operation 权威记录, 不拥有 Agent 的决策状态. Agent Loop 可以在任意获准且能访问其数据和能力的 Worker 上运行; AgentId 不绑定 AuthorityId 或 WorkerId. 当前 Loop 结束后可以选择另一 Worker, 运行中迁移和自动故障接管不在首版承诺内.

Node 是组装与生命周期容器, 不是另一种执行授权方. 用户分别选择稳定身份, 承载角色以及接入的基础设施. 稳定 NodeId/AuthorityId/WorkerId 可由用户提供或首次生成后保存; 运行实例 ID 每次启动更换. UUID 不是地址, 认证凭证或接管许可.

Zenoh 连接/Router, 数据库, 日志/指标/追踪出口分别配置, 不因为监听网络就自动充当 Router, 不因为连接数据库就成为 Authority. 不接入持久化时须显式标明易失模式. 当前默认 CLI 每次随机生成逻辑身份, 自动保存身份仍待实现.

UI, Agent 执行, Authority, Worker 与存储可以分节点. 远程 SQLite 需要存储服务处理消息并在本地通过 Diesel 完成事务, 不把 SQLite 文件当作可直接消息访问的对象. 首版优先本地组合与本地 SQLite, 不要求建设远程存储服务.

## 通信与调度

同进程使用类型化调用或通道, 保留相同授权和证据检查; 跨进程由 Adapter 编码, Zenoh 为当前主要传输. WIT/Protobuf 只借鉴描述类型和 Interface 的格式, 未决定接入生成器, 编码, gRPC 或 Wasm 工具链.

依赖顺序由事实决定: 请求可以并发接收, 独立 Operation 可以并发执行, 数据库提交可以异步等待. 只有依赖某次提交成功的动作需要等待它. 不把外部执行放入数据库事务, 不让同步数据库操作阻塞消息与 Loop 驱动线程.

不假设所有 Operation 共用一个并发名额池, 不新增组合执行池或容量原语. `await` 保持父 Operation 未完成, 允许接收消息和其他执行继续推进. 具体执行器可以限制 Shell 进程数或 LLM 请求数; 若配置了此类限制, 需避免父执行等待时耗尽子执行必需资源. 这不撤销现有原型的有界队列和记录限制, 也不要求无界创建任务.

本地组合, 长连接和订阅复用, 携带完整 Report 的通知, 独立调用并行以及事务批处理用于降低额外开销. 不要求每次调用依次进行提交, 订阅, 查询三个网络往返. 性能需基准实测, 没有延迟保证或吞吐目标数值.

## 明确的取舍

- 保留统一 Operation/Attempt/Report 表达 Loop 与 Codemode 的执行结果, 不另建 Task 生命周期或强制独立 Tool Reply 账本.
- Agent 独立持久化, 通过 ID 引用执行事实. 不要求 Event Sourcing, Event Bus, Actor 框架, Workflow DSL 或全局分布式事务.
- LLM 上下文是 Agent 选定事实的投影; 不删除原证据, 不让投影规则定义执行实体的生命周期.
- fork 是数据复制与新身份, 没有运行依赖型历史继承链, 不复制活动执行.
- Sub-Agent 与 MCP Sampling 的具体协议暂缓; 统一调用与观察 Interface 不以它们为前提. 不承担 systemd 式服务维持, 任意代码检查点或外部影响 exactly-once 保证.

旧讨论中的"Loop/Turn 一律在 Operation 外", "Codemode 必须使用另一套权威返回记录", `Advancing`, 公开 `AgentRuntime` 实体和强制双执行池均已撤回. 最终状态名采用 `Processing`; 不需要额外的 `WaitingContext` 或独立领域"等待观察状态机".
