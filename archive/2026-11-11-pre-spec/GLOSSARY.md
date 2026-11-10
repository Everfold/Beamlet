# Beamlet 执行与 Agent 领域

Beamlet 表达执行请求, 授权与证据, Beamlet-Agent 在其上表达持续存在的 Agent 和组合执行. 本表定义领域概念, 实现状态与契约见 [设计入口](spec/design/README.md).

## Language

**Node**:
承载基础设施和业务角色实例的运行容器, 不因承担网络监听而获得执行授权权力.

**Client**:
提交执行意图, 查询或观察执行与角色事实的参与方; 可以只观察, 不负责创建执行授权, 也不因具有 Client 能力成为独立服务.
_Avoid_: Observer 作为独立参与方

**Authority**:
持有 Operation 权威记录, 决定执行授权并收录执行证据的角色.

**Worker**:
接纳指定 Authority 的执行授权, 执行工作并保留执行证据的角色.
_Avoid_: Work, Runner 作为对外角色名

**Operation**:
具有独立身份的不可变逻辑请求, 相同任务内容不代表同一请求.

**Operation capability**:
可被请求调用的行为契约, 定义所需输入, 可返回结果及调用约束; 不等于一次 Operation 或对调用方的授权.

**Tool**:
面向 LLM 的调用接口, 由宿主将调用映射为具体处理行为; 与 Operation 不要求一一对应.

**Agent**:
通过自身的 Loop Operation 参与执行, 具有配置, 输入, 历史关联和推进状态的独立实体; 相同配置的新创建和 fork 均产生不同 Agent.
_Avoid_: Authority/Worker 本地组合入口作为领域 Agent

**Agent Loop**:
Agent 根据输入与已取得结果持续决定后续调用的执行逻辑, 由该 Agent 对应的 Operation 表达. 正常结束要求其负责的子执行与结果处理全部完成, Loop 返回后数据可保留.

**Agent 输出**:
Agent 已选择向应用或 UI 发布的回答, 可以在 Loop 尚未结束时产生; 不等同于整个 Loop 的最终 Report.

**LLM Request**:
以固定模型配置, 已构造输入和工具声明请求模型生成内容的一次执行, 由独立 Operation 表达.
_Avoid_: Agent 的一次调用

**Codemode**:
执行代码以组合多个调用并返回结果的能力; 一次代码执行由 Operation 表达, 内部工具调用可以形成独立 Operation.

**Preset**:
可被多个 Agent 引用的基础配置, 具体生效内容通过固定版本引用与 Agent 自己的 overrides 确定.

**Fork**:
从完整一致且无未处理执行责任的 Agent 数据创建新身份与独立副本; 来源仅用于审阅, 不形成运行依赖或复制活动执行.

**执行来源**:
实际发起提交的 Client 或授权执行; 对执行发起的提交区分具体 Operation/Attempt, 来源关系不授予额外权限或所有权.

**触发依据**:
引出某次提交的具体证据及其中的调用位置, 不等于实际提交方或执行授权.

**Attempt**:
针对指定 Operation 的一次执行授权, 不是消息投递次数.

**Report**:
有关一次授权执行的证据, 独立于 Authority 是否采用及业务是否成功.

**ImpactStatus**:
对某次执行在声明范围内是否影响外部世界的证据结论, 区分确认无影响, 已观察影响与无法确定; 与执行成功或失败独立.
_Avoid_: Effect, 无限定的 Status 或 Evidence

**SubmissionKey**:
在指定 Authority 内关联同一次提交的稳定去重键, 不等于 Operation 的身份.

**稳定角色身份**:
跨启动标识同一逻辑角色的身份, 与网络地址, 运行实例及授权凭证分别存在.

**角色实例**:
角色的一次运行身份, 与跨启动保持的逻辑角色身份不同.

**等待条件**:
当前 Loop 步骤继续推进前必须满足的结果或输入条件, 不等于所有尚未处理的调用集合.

**跟踪中的调用**:
Agent 或组合执行仍承担结果处理责任的调用; 暂不阻塞当前步骤也不免除完成前的处理责任.
