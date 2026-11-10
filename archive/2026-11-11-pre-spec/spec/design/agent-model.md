# Agent 实体与状态

状态: 已确认的语义约束与待实现的数据职责, 2026-11-10. 伪类型不承诺最终 Rust Interface, SQL 表名或网络编码. 架构入口见 [设计入口](README.md).

## Agent 与 Loop

Agent 通过代表其 Loop 的 Operation 参与执行, 配置, 输入, 关联记录和推进状态独立持久化. 一个 Agent 对应自己的 Loop Operation, 其中可以包含多次 LLM Request 和 Tool/Codemode Operation. 不将 LLM Request 称为 Agent 的一次调用, 不将每个 Turn 再包装为 Operation.

新创建请求产生新 Agent 及其 Loop Operation, 相同配置不代表同一个 Agent; fork 同样产生新 Agent. 不提供将另一个独立 Loop Operation 绑定到原有 Agent 并共同推进其数据的入口. 同一创建提交的重传由原 SubmissionKey 关联回原 Operation, 不创建第二个 Agent. AgentId 与 OperationId 是否复用同一值属于表示细节, 本契约不要求增加或合并 ID 类型.

Loop 的开始须固定输入与配置版本, 正常完成遵守下文的统一规则. 新输入投递给尚未结束的 Agent Loop, 默认排队, 不因一条用户消息创建新的 Loop Operation. 是否允许输入介入当前处理步骤由显式控制策略决定; steer 作为后续能力, 不修改已接纳 Operation 的原始请求.

Agent 的运行位置是执行 Loop 的 Worker, 不要求与 Authority 同进程或绑定. Worker 通过执行宿主提供的 Client Interface 提交, 查询和观察子调用. Agent 运行入口是加载数据和驱动事件的代码, 不新增具有身份或独立持久化记录的 `AgentRuntime` 实体.

## 数据职责

```text
Agent {
  agent_id
  origin: Created | ForkedFrom { agent_id }
  config: { preset_ref, overrides }
  inputs
  history
  loop_progress
}

LoopProgress {
  loop_binding
  phase: Processing | Waiting | Suspended
  input_cursor
  pending_submissions
  tracked_calls
  wait_condition
  policy_state
}

PlannedInvocation {
  local_call_key
  target_authority_id
  submission_key
  immutable_request
  origin
  binding: Unbound | Bound { execution_ref }
}

ExecutionRef { authority_id, operation_id, attempt_id }
ReportRef { authority_id, report_id }
```

`loop_binding` 关联该 Agent 所对应的 Loop Operation 与执行授权, 不是竞争已有 Agent 的占用记录. 提交方在接纳回执到达前保存原 SubmissionKey, 接纳后补齐执行引用; 创建或加载 Agent 数据须复用该绑定, 不因重复处理创建另一份实体. 不需要独立的活动 Loop 预留机制.

| 数据 | 含义与权威来源 |
| --- | --- |
| inputs | 用户输入及消费顺序, 不能从 Operation 参数可靠反推; 已消费与未消费必须可区分 |
| history | 有序输入/执行引用, 选定 Report, 已发布回答及其稳定位置, 实际采用的数据关系及明确处置; 内容可以通过 ID 查询 |
| pending_submissions | 已决定但尚未确认接纳的完整请求, 目标 Authority 与原 SubmissionKey |
| tracked_calls | 调用来源, 执行绑定, 已观察/采用结果及处理位置; 不复制执行账本的权威状态 |
| policy_state | 继续 Loop 所必需的策略进度; 不保存任意语言调用栈 |

不强制采用 `AgentRecordEntry` 事件日志, 不为每个内部状态变化建立新的业务实体. 相同 capability 参数可以对应不同的调用, 不能以参数推导身份, 顺序或已处理状态. 输入和关系可以拆表或序列化保存, 最终布局由一致更新与查询需要决定.

`origin` 仅用于审阅, 加载 Agent 不沿来源链读取或重建历史. Agent 数据版本号 `revision` 可用于条件写入与快照校验, 不是 Loop 轮数或 preset 版本; 是否使用此字段取决于选定的并发控制实现, 不强制作为领域原语.

## Preset 与可复现输入

配置表达为 `preset_id + 固定版本 + overrides`. 引用固定到不可变版本, 更新 preset 不得静默改变既有执行, 历史或 fork 副本的含义. 修改 Agent 的有效配置应形成明确记录, 已提交请求不重新合成. 版本的具体编码由实现确定.

Override 按字段覆盖, 数组整体替换, 未配置与明确清空分别表达. 具体字段路径和清空编码在实现时确定并测试, 不隐式承诺任意深度合并. 每次 LLM Operation 保存最终实际请求, 包括模型配置, 构造好的消息和工具声明, 从而不依赖未来默认值重建历史. 凭据由运行环境提供, 不嵌入持久请求或模型输入.

## 创建身份与执行去重

两个客户端分别以不同 SubmissionKey 提交相同配置, 产生不同 Operation 和不同 Agent, 不会按配置或来源合并为同一实体. fork 副本有自己的身份与执行绑定, 不借用来源 Agent 的可变推进记录.

同一 SubmissionKey 的重传复用原 Operation; 同一 Attempt 的重复交付由 Worker 去重, 不再启动一个执行循环. 接收输入的代码向该 Agent 交付消息, 不直接启动另一个 Loop 改写它. 这是创建和执行契约, 不需要再加入 Agent 级分布式锁, 租约或启动抢占表.

恢复的是原有执行, 不是用新创建请求继续同一个实体. Worker 重启后的旧实例隔离与恢复授权属于所有 Operation 共用的执行底座问题, 继续遵守 [持久化与恢复](persistence-recovery.md), 不另建 Agent 专用执行权系统. 同一执行内部仍需一致地处理输入, 结果与持久状态更新.

## 状态机

Processing, Waiting, Suspended 描述同一个 Loop 内的推进状态. 若保留 Ready, 它只表达本执行已准备好处理输入, 不表示可以为原 Agent 创建另一个 Loop. 完整状态编码仍是草案; Operation 的完成由自身结果表示. `Waiting` 表达现有执行流程尚待结果或必要输入, 期间仍接收消息, 不阻塞线程或关闭收件箱. 所有工作处理完毕后正常结束, 不另外进入为了等待未来新任务而保持存活的状态.

| 当前状态 | 输入或条件 | 数据与动作 | 后续状态 |
| --- | --- | --- | --- |
| 创建后或 Ready | Loop 的 Attempt 获准执行 | 按原执行绑定初始化或加载 Agent 数据, 处理输入 | Processing |
| Processing | 待发送调用已保存 | 异步提交; 接纳回执绑定执行 ID | 有其他可做工作时 Processing, 否则 Waiting |
| Waiting | 合法 Report 到达 | 保留观察, 校验预期关联并选择采用结果; 工具结果进入后续 LLM 输入准备 | 可推进时 Processing, 否则 Waiting |
| Processing | LLM 返回完整调用提议 | 校验行为版本, 参数, 目标, 权限与预算; 保存派生调用 | Processing 或 Waiting |
| 任一活动状态 | 重复 Report/回执 | 去重, 补齐原绑定, 不重复派生或回退完成事实 | 保持状态或处理尚未消费的事实 |
| Processing / Waiting | 超时, 预算耗尽, 暂停请求 | 记录原因, 按策略等待或暂停; 不推断未执行 | Waiting 或 Suspended |
| Suspended | 新证据到达 | 保留观察, 不隐式恢复主动调用 | Suspended |
| Suspended | 明确恢复且执行权有效 | 查询补齐, 继续已记录决定 | Processing 或 Waiting |
| Processing | 正常完成条件成立 | 保存最终结果与完成意图, 幂等交付 Loop Report | 本执行结束, 数据保留供查询与复制 |

没有必经的 `WaitingContext`. 本地历史选择和请求格式转换是提交准备; 若上下文压缩需要独立 LLM/工具调用, 该调用进入普通跟踪和等待机制.

正常完成采用统一规则: 该 Agent 负责的子 Operation 全部结束, 工具结果已交给后续 LLM Operation 处理, 最后一次 LLM 已完成且没有新的调用提议, 同时没有待提交动作或未处理输入/结果. 范围是该 Agent 负责的子执行, 不包含正在判定完成的 Loop 自身或无关 Agent 的 Operation. 不按交互型/任务型区分结束条件, preset 不增加此类生命周期开关.

异步工具返回后, 保存结果及后续 LLM 处理责任, 构造并提交下一次 LLM Request. 多个工具结果可以进入同一次后续 LLM Request. 从工具完成到后续 LLM 提交之间仍有待处理工作, 不能因为此刻没有运行中的子 Operation 就结束 Agent. LLM 提前返回的一段文本也不免除这些责任. 暂停和停止仍按下文的控制语义处理.

最终 Agent 数据与 Loop Report 可能属于不同存储事务; 在可恢复的结果记录和交付责任成立前不能清空活动状态. 实现须处理"最终 Report 已准备但尚未被 Authority 保留"的窗口, 不能再次执行整个 Loop.

等待条件是对当前推进所需结果的条件, 跟踪集合则是仍需承担结果处理责任的所有调用. 例如跟踪 A/B/C, 当前只等待 A/B, C 可以后台执行; C 未处理前不能正常完成整个 Loop. 不在 Operation 上固定 `blocking` 布尔值, 同一调用可以先后台运行, 后来再被等待.

## 已发布回答

Agent 在历史关联中保存向应用/UI 发布的回答及其稳定位置, 内容可引用已有 LLM Report 中的块. 保存回答后即可通知 UI, 无需等待整个 Loop 的最终 Report. 发布回答与结束 Loop 分别记录; 异步工具仍在执行时可以发布当前回答, 之后再发布处理工具结果得到的回答.

查询返回已保存的回答, 通知用于及时更新; UI 按稳定位置合并去重, 断线或遗漏通知后通过查询补齐. 重复处理同一 LLM Report 复用原发布记录. 本阶段只约定完整回答的保存和观察, token 流式展示另行接入. 具体接口见 [UI 接收 Agent 回答](execution-client.md#ui-接收-agent-回答).

## 暂停, 停止与异常

暂停主动推进, 请求停止, 请求取消子执行与正常完成不是同一件事. 停止默认不再发起新的业务调用, 继续收取已有执行的结果; 对支持取消的 Tool 可以按显式策略发出取消请求, 不默认为强制取消传播. 等待超时后保留未确认记录, 不宣称工具已停止. Tool 是否支持取消由 capability 契约决定, 请求取消不证明已经停止.

正常完成不能遗留当前 Loop 仍需处理的异步工具. 异常退出或明确终止可以留下未确认执行, 但必须保留原因, 子执行引用和后续处置责任; 不生成虚假的成功或无影响结论. `stop` 的公共消息, 终止结果编码和处置交接 Interface 在实现前定案, 不默认为强杀或自动取消传播.

持久服务由外部管理器维持. 启动服务的 Operation 在完成启动契约后返回服务标识或地址, 不以服务退出作为 Agent Loop 的完成条件. Beamlet-Agent 不承担 systemd 式托管.

## Fork

fork 复制完整一致的 Agent 数据, 分配新 AgentId, 标记直接来源, 以新的 Loop Operation 参与执行. 配置复制的是不可变 preset 引用和 overrides; 有序关联记录与结果采用位置也复制. 副本不继承来源 Agent 的当前 Loop 执行绑定, 历史中的相同 ID 仍指向同一份执行事实, 不重新执行历史工具, 不设置运行依赖型历史前缀指针.

`can_fork` 是独立检查, 不能只看枚举状态或能否构造 prompt:

1. 没有活动的执行延续, 待提交动作或尚未处理的输入决定/结果.
2. 没有仍需由该 Agent 跟踪处理的异步执行.
3. 未确认历史事项已有明确处置, 引用与未知事实完整保留; 处置不宣称远端已停止.
4. 数据读取和创建副本具有一致快照, 与启动/推进互斥或通过版本校验保证一致.

原始未消费输入如何排队属于数据复制语义, 不能在 fork 时静默变为"已消费"或自动发送两份相同待提交命令. 首版仅在执行责任稳定时复制, 不支持从运行中的任意历史位置分叉.

副本独立新增记录, 不跟随原 Agent 更新. 锁, 连接, 订阅, future, 活动 Attempt 和写入权不复制. 复制的历史调用标识保持为历史引用, 新调用使用新 Agent 自己的身份空间. fork 不复制文件系统或外部服务状态, 也不继承查询或执行权限的证明.
