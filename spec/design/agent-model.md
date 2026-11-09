# Agent 实体与状态

状态: 已确认的语义约束与待实现的数据职责, 2026-11-10. 伪类型不承诺最终 Rust Interface, SQL 表名或网络编码. 架构入口见 [设计入口](README.md).

## Agent 与 Loop

Agent 是独立持久化的实体, 包含配置, 输入, 关联记录和推进状态. Agent Loop 的一次执行由 Operation 表达, 可以包含多次 LLM Request 和 Tool/Codemode Operation. 不将 LLM Request 称为 Agent 的一次调用, 不将每个 Turn 再包装为 Operation.

Loop 的开始须固定启动输入范围, 配置版本与完成策略. 新输入默认记录到同一 Agent 的输入队列; 已有 Loop 时不隐式创建第二个 Loop. 是否允许输入介入当前 Loop 由显式控制策略决定. 首版默认排队, steer 作为后续能力, 不修改已接纳 Operation 的原始请求.

Agent 的运行位置是执行 Loop 的 Worker, 不要求与 Authority 同进程或绑定. Worker 通过执行宿主提供的 Client Interface 提交, 查询和观察子调用. Agent 运行入口是加载数据和驱动事件的代码, 不新增具有身份或独立持久化记录的 `AgentRuntime` 实体.

## 数据职责

```text
Agent {
  agent_id
  origin: Created | ForkedFrom { agent_id }
  config: { preset_ref, overrides }
  inputs
  history
  active_loop
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

`active_loop` 在建立 Loop 的提交意图时就必须预留, 能表达尚未拿到 Operation/Attempt ID 的启动状态. `loop_binding` 可以是原提交键或已确认的 ExecutionRef. 不能等回执到达后才防止第二次启动. 具体类型与写入方案见单写者约束.

| 数据 | 含义与权威来源 |
| --- | --- |
| inputs | 用户输入及消费顺序, 不能从 Operation 参数可靠反推; 已消费与未消费必须可区分 |
| history | 有序输入/执行引用, 选定 Report, 实际采用的数据关系及明确处置; 内容可以通过 ID 查询 |
| pending_submissions | 已决定但尚未确认接纳的完整请求, 目标 Authority 与原 SubmissionKey |
| tracked_calls | 调用来源, 执行绑定, 已观察/采用结果及处理位置; 不复制执行账本的权威状态 |
| policy_state | 继续 Loop 所必需的策略进度; 不保存任意语言调用栈 |

不强制采用 `AgentRecordEntry` 事件日志, 不为每个内部状态变化建立新的业务实体. 相同 capability 参数可以对应不同的调用, 不能以参数推导身份, 顺序或已处理状态. 输入和关系可以拆表或序列化保存, 最终布局由一致更新与查询需要决定.

`origin` 仅用于审阅, 加载 Agent 不沿来源链读取或重建历史. Agent 数据版本号 `revision` 可用于条件写入与快照校验, 不是 Loop 轮数或 preset 版本; 是否使用此字段取决于选定的并发控制实现, 不强制作为领域原语.

## Preset 与可复现输入

配置表达为 preset 引用加 overrides. 引用必须固定到不可变版本: 可以是版本化 preset ID, 或 `preset_id + revision`, 两种编码尚待选择. 更新 preset 不得静默改变既有执行, 历史或 fork 副本的含义. 修改 Agent 的有效配置应形成明确记录, 已提交请求不重新合成.

Override 的具体合并规则须在实现前固定并测试, 尤其是字段缺省, 显式清除与数组覆盖的区别. 当前不承诺任意深度合并. 每次 LLM Operation 保存最终实际请求, 包括模型配置, 构造好的消息和工具声明, 从而不依赖未来默认值重建历史. 凭据由运行环境提供, 不嵌入持久请求或模型输入.

## 单写者与执行权

同一个 Agent 同时只允许一个活动 Loop 的合法推进者. 两个不同 Operation 即使都获 Authority 授权, 也不能同时修改同一 Agent. 约束由 Agent 存储拥有, 不能只靠 Worker 进程内锁或单个 Authority 的局部观察.

启动 Loop 时, 需要原子地预留活动 Loop 并保存启动提交意图. 获得执行绑定后将预留与该授权关联. 所有推进更新检查当前归属; 其他输入可以被持久接收, 但不产生第二个主动 Loop. 抢占失败不能在外部执行后才被发现.

实现前须选择能跨执行进程生效的归属校验方式, 如条件更新与持久代次. 这里规定不变量, 不预先引入租约服务. 数据库拒绝旧状态写入并不自动阻止旧执行者调用外部工具; 调用接纳和执行侧的旧授权隔离同样需要处理. 旧进程仅失联时不自动接管, 详见 [持久化与恢复](persistence-recovery.md).

## 状态机

`Ready` 表示没有活动 Loop 的推进责任, 可以启动新 Loop. 有活动 Loop 时使用 Processing, Waiting, Suspended. `Waiting` 期间仍接收输入和执行观察, 不阻塞线程或关闭收件箱.

| 当前状态 | 输入或条件 | 数据与动作 | 后续状态 |
| --- | --- | --- | --- |
| Ready | 启动请求 | 原子预留 Loop 与待提交请求; 使用 Client 提交 | Processing, 启动交付尚待确认时可 Waiting |
| Processing | 待发送调用已保存 | 异步提交; 接纳回执绑定执行 ID | 有其他可做工作时 Processing, 否则 Waiting |
| Waiting | 合法 Report 到达 | 保留观察, 校验预期关联; 由策略选择是否采用 | 可推进时 Processing, 否则 Waiting |
| Processing | LLM 返回完整调用提议 | 校验行为版本, 参数, 目标, 权限与预算; 保存派生调用 | Processing 或 Waiting |
| 任一活动状态 | 重复 Report/回执 | 去重, 补齐原绑定, 不重复派生或回退完成事实 | 保持状态或处理尚未消费的事实 |
| Processing / Waiting | 超时, 预算耗尽, 暂停请求 | 记录原因, 按策略等待或暂停; 不推断未执行 | Waiting 或 Suspended |
| Suspended | 新证据到达 | 保留观察, 不隐式恢复主动调用 | Suspended |
| Suspended | 明确恢复且执行权有效 | 查询补齐, 继续已记录决定 | Processing 或 Waiting |
| Processing | 正常完成条件成立 | 保存最终结果与完成意图, 幂等交付 Loop Report | 完成责任安全交接后 Ready |

没有必经的 `WaitingContext`. 本地历史选择和请求格式转换是提交准备; 若上下文压缩需要独立 LLM/工具调用, 该调用进入普通跟踪和等待机制.

正常完成必须没有待提交动作, 未处理的必要结果或仍需回收的异步工具. LLM 的一段文本不能单独满足完成条件. 最终 Agent 数据与 Loop Report 可能属于不同存储事务; 在可恢复的结果记录和交付责任成立前不能清空活动状态. 实现须处理"最终 Report 已准备但尚未被 Authority 保留"的窗口, 不能再次执行整个 Loop.

等待条件是对当前推进所需结果的条件, 跟踪集合则是仍需承担结果处理责任的所有调用. 例如跟踪 A/B/C, 当前只等待 A/B, C 可以后台执行; C 未处理前不能正常完成整个 Loop. 不在 Operation 上固定 `blocking` 布尔值, 同一调用可以先后台运行, 后来再被等待.

## 暂停, 停止与异常

暂停主动推进, 请求停止, 请求取消子执行与正常完成不是同一件事. 默认保守行为是停止发起新调用, 继续收录已有证据并明确处理未完成责任. Tool 是否支持取消由 capability 契约决定, 请求取消不证明已经停止.

正常完成不能遗留当前 Loop 仍需处理的异步工具. 异常退出或明确终止可以留下未确认执行, 但必须保留原因, 子执行引用和后续处置责任; 不生成虚假的成功或无影响结论. `stop` 的公共消息, 终止结果编码和处置交接 Interface 在实现前定案, 不默认为强杀或自动取消传播.

持久服务由外部管理器维持. 启动服务的 Operation 在完成启动契约后返回服务标识或地址, 不以服务退出作为 Agent Loop 的完成条件. Beamlet-Agent 不承担 systemd 式托管.

## Fork

fork 复制完整一致的 Agent 数据, 分配新 AgentId, 标记直接来源, 创建独立运行关联. 配置复制的是不可变 preset 引用和 overrides; 有序关联记录与结果采用位置也复制. 相同 ID 仍指向同一份执行事实, 不重新执行历史工具, 不设置运行依赖型历史前缀指针.

`can_fork` 是独立检查, 不能只看枚举状态或能否构造 prompt:

1. 没有活动的执行延续, 待提交动作或尚未处理的输入决定/结果.
2. 没有仍需由该 Agent 跟踪处理的异步执行.
3. 未确认历史事项已有明确处置, 引用与未知事实完整保留; 处置不宣称远端已停止.
4. 数据读取和创建副本具有一致快照, 与启动/推进互斥或通过版本校验保证一致.

原始未消费输入如何排队属于数据复制语义, 不能在 fork 时静默变为"已消费"或自动发送两份相同待提交命令. 首版仅在执行责任稳定时复制, 不支持从运行中的任意历史位置分叉.

副本独立新增记录, 不跟随原 Agent 更新. 锁, 连接, 订阅, future, 活动 Attempt 和写入权不复制. 复制的历史调用标识保持为历史引用, 新调用使用新 Agent 自己的身份空间. fork 不复制文件系统或外部服务状态, 也不继承查询或执行权限的证明.
