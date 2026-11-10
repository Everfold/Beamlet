# Operation 组合与 Agent Loop

状态: 2026-11-10 更新的目标契约, 尚未实现. 本文替换旧版组合方案. Agent 是独立实体, Agent Loop 的执行是 Operation; 每次 LLM Request, Codemode 与具体工具调用使用相同执行底座. 实体与状态见 [Agent 模型](agent-model.md), 结果获取见 [执行 Client](execution-client.md).

## 请求与完成边界

| 执行 | Operation 输入 | Report 表达什么 |
| --- | --- | --- |
| Agent Loop | 新建配置或 fork 副本数据, 明确的输入 | 该 Agent 的 Loop 返回结果及所采用证据; 持久数据可继续保留 |
| LLM Request | 固定模型配置, 已构造的消息, 工具声明 | 本次模型返回的完整内容或失败证据 |
| Codemode | 代码, 输入, 固定执行器版本与获准能力范围 | 本次代码执行的结果或错误, 以及使用的执行证据 |
| 普通 Tool | capability 版本与明确参数 | 本次工具执行的结果及外部影响证据 |

Operation 原始定义不可变. Loop 收到补充输入时将其作为独立输入事实记录, 不改写原始请求. Turn 仅用于组织交互, 不额外包一层 Operation. 不用"Agent 的一次调用"称呼 LLM Request.

Loop Operation 关联其自己的 Agent, 不接受任意指定一个已存在 Agent 来启动第二个独立 Loop. 相同配置的新提交建立不同 Agent, fork 也使用新身份与执行绑定. 该创建的重传与恢复沿用原记录, 不把它解释为新建实体. 具体 ID 生成与关联编码由实现收敛.

正常返回意味着该执行的完成契约已满足. 异步工具允许父执行先做其他工作, 返回结果后仍须进入后续 LLM Operation. Agent 在其负责的子执行及结果处理全部完成, 没有新的调用或待处理输入时正常结束, 不以交互型/任务型区分. 工具结束到后续 LLM 提交之间的处理责任同样阻止提前完成. 异常终止须明确遗留责任与未知执行, 不推断子调用已经停止. 服务启动工具以启动步骤的结果返回, 不负责服务长期存活.

## 完整调用链

```text
Agent A, Loop Operation O / Attempt O1
  -> LLM L1 / Attempt L1-1
       -> LR1: Text + Call(codemode, provider_call_id=c1)
  -> Codemode C / Attempt C1
       -> Tool F1 / Attempt F1-1 -> FR1
       -> Tool F2 / Attempt F2-1 -> FR2
       -> CR: 汇总值, 使用的 FR1/FR2 引用
  -> 构造下一次 LLM 输入
  -> LLM L2 / Attempt L2-1
       -> LR2: 最终输出
  -> 检查完成条件
  -> OR: Loop 返回结果与采用的证据引用
```

LLM 只返回调用提议. 宿主校验后才提交 C, Authority 再授权 C1. C 的工具接口经宿主提交 F1/F2, 不直接创建授权或跳过权限检查. 返回给模型的 `c1` 结果来自 CR, 不强制展开 FR1/FR2 为模型未请求过的工具调用.

同进程可直接类型化交付, 仍保留接纳, 授权和结果校验. 不要求为了记录内部子调用而经过网络.

## Tool 与 capability

Tool 是面向 LLM 的调用接口, capability 是执行行为契约, Operation 是一次执行请求. Tool 可以映射一个 Operation, 也可以由组合逻辑处理; 不把 provider 的 Tool Call 标识当成 OperationId.

本阶段普通工具映射具体 capability, Codemode 映射代码执行 Operation. 不再为 Codemode 建立另一套权威 Tool Reply 结果账本. 如果某个本地处理没有对应 Operation, 它的输入关联与返回必须由 Agent 明确保存, 不能伪造 Report. 不因这种可扩展性而预先建设通用 Task 框架或 Agent 间调用协议.

作为执行 capability 显式暴露的普通计算, 包括拼接, 过滤, 解析或摘要, 均通过 Operation 执行. 执行器内部的变量赋值, 语言求值, 协议解码与账本更新不递归成为新 Operation. 普通历史投影不必单独创建上下文 Operation; 显式暴露的压缩/摘要调用则使用普通执行路径.

## 关联数据

以下是数据职责示意, 不是强制 JSON 或最终 Rust 类型:

```text
Invocation { capability_ref, input }

LlmOutput {
  blocks: [Text | ProviderReasoning | Call]
  completion: Complete | Incomplete
}

Call { provider_call_id, invocation }

ExecutionRef { authority_id, operation_id, attempt_id }
ReportPartRef { authority_id, report_id, block_index }

InvocationOrigin {
  submitted_by: ClientSource | ExecutionRef
  local_call_key
  triggered_by: Optional<ReportPartRef>
  agent_id: Optional<AgentId>
}
```

稳定 AuthorityId 用于定位执行账本, 运行实例绑定仍由授权与消息协议保留. 实际调用身份由宿主绑定; 来源和 AgentId 仅用于关联, 不证明权限. ClientSource 的最终编码待 Client Interface 定案, 初始提交无需伪造父 Attempt.

| 提交 | 实际执行来源 | 触发依据 |
| --- | --- | --- |
| Loop O | Client 启动请求 | 输入记录 |
| L1 | O/O1 | Loop 策略, 可无 Report 触发 |
| C | O/O1 | LR1 中的调用块 |
| F1/F2 | C/C1 | 代码的独立调用位置, 可无 Report 触发 |
| L2 | O/O1 | Loop 策略; 实际采用 LR1/CR 另记为数据依赖 |

`submitted_by`, `triggered_by` 和实际使用的 Report 是三种关系, 不能压成单个 parent_id. LLM 已返回后, 工具不是由已完成的 LLM Attempt 主动提交. 反向查询可从关系生成, 不要求修改原 Operation 追加可变 children 数组.

同一执行中的两次相同参数调用仍是两次独立调用. `local_call_key` 由宿主为逻辑调用分配并保存, 重传和重复消费复用原位置, 不按参数哈希去重. provider_call_id 只在其模型输出范围内配对请求与回复; block_index 指不可变规范化输出的位置, 不随 prompt 裁剪重新编号.

## 从结果派生调用

1. 校验 Report 的 Operation/Attempt/Authority 关联, 完整性与输出契约, 选择并保存本轮采用的 Report.
2. 校验调用参数, capability 版本, 目标, 权限和执行预算. 相同调用块重复到达时查询原派生记录.
3. 一致地保存消费位置与派生请求: 来源, 本地调用位置, 目标 Authority, 固定 Invocation, 稳定 SubmissionKey.
4. 提交保存的请求; 回执返回后补齐 Operation/Attempt 绑定. 合法 Report 先到时也可补齐绑定, 不因等回执而阻塞既有派发.
5. 收录调用的选定结果, 更新等待条件. 相同或附加 Report 不自动派生第二组调用.

这种记录保存的是执行决定, 不意味着可以任意重放代码. 相同键不同定义必须拒绝, 不覆盖已有映射. 每次重传沿用原目标与身份, 不因超时新建 Attempt 或改派. 通知, 查询和重连返回可重复, 处理必须幂等.

## LLM 输入与证据

```text
Agent 的输入与有序引用
  + 选定的 LLM Report
  + 对应的 Tool / Codemode Report
  + 必要的明确处置事实
  -> 选择与裁剪
  -> provider 格式转换
  -> 固定的下一次 LLM Request
```

Agent 决定顺序和采用关系, Beamlet 提供执行内容. 历史不等于 prompt, prompt 也不是隐藏的可变权威状态. 实际发出的请求需要可追溯, 不能恢复时按新 preset 或新模板重新解释原调用.

投影必须满足具体 provider 的调用/结果配对约束. Codemode 汇总可以隐藏内部细节以节省 token, 但不能删除原证据, 将未知改成成功, 或把尚未完成的工具写成已完成. provider 返回的 reasoning 内容可以记录, 不承诺读取模型未公开的内部推理. 流式片段和不完整参数不能直接授权工具调用.

异步工具如需先向模型返回执行中信息, 必须由工具契约和 provider Adapter 明确表达, 最终结果另行关联, 不能伪造完成 Report. 如果 provider 不支持当前的交互组合, 就等待必要结果或明确报错, 不生成非法消息序列.

## Codemode 执行与恢复限制

Codemode 执行器运行固定代码与输入, 在受限环境中提供工具调用 Interface. `tools.read_file(...)` 等调用经宿主记录和提交 Operation, Report 到达后将结果交回执行器. 凭据和宿主内存不因代码运行自动可见; 允许能力与外部资源范围由配置授予.

父 Codemode 和子工具各有自己的 Report. 父执行失败不能抹掉已发生的子工具影响, 父执行返回不隐含子执行取消. 父 Report 的影响结论必须遵循 capability 声明的范围, 不自动沿树聚合, 不能因父代码"只做编排"就宣称无影响.

首版正常返回前回收本次代码需要处理的所有工具结果, 不把未完成子工具隐式遗留为后台服务. 进程内 await 可以继续代码, 持久化子调用记录不等于保存任意变量和控制流. 执行中崩溃时保留中断/未知事实与子执行引用, 不自动重跑整段代码. 检查点或受约束的代码重放另行设计, 不是当前实现前提.

Agent Loop 可以通过显式阶段和决定恢复; Codemode 只能恢复到其执行器明确支持的范围. 实例更替后的授权恢复仍必须先满足 [恢复契约](persistence-recovery.md), 不能因为有一份 LoopProgress 就启动第二个推进者.

## 调度边界

父 Operation 在 await 子 Report 时保持未完成, 消息接收与其他执行仍能推进. 不假设全局并发名额, 不要求独立的编排池. 具体执行器若实施资源限制, 负责验证子调用可取得所需资源, 不让等待的父执行耗尽该资源. 旧原型的有界调度不是领域约束, 也不能未经验证直接用于任意嵌套.

Sub-Agent, Agent 间请求协议与 MCP Sampling 暂不设计. 保留通过受控 Client 提交和观察执行的组合能力, 不为未来协议预建实体.
