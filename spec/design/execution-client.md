# 执行 Client 与结果观察

状态: Interface 的语义契约, 2026-11-10. 通用 Authority 已实现按 SubmissionKey 的内存接纳, Console 已复用该实现, 仍只有固定 Hello 提交及重复提交读取原结果的能力; 通用 Client 查询/订阅 SDK 尚未实现. [回执契约](receipts.md)描述当前易失原型.

当前库入口为 `Authority::submit_keyed(key, task, allocate)`: 定义按类型 P 的 Eq 比较, 传输信封和临时回复地址由 Adapter 处理. 只有新键通过记录容量检查后才调用 allocate 生成 OperationId/AttemptId; 同键同定义返回原授权, 冲突或容量不足不生成身份. 已用 ID 被拒绝, 不把新键关联到已有执行. `check_submission(key, &task)` 返回已接纳执行的当前观察或定义冲突, 未知键返回空值; 它不修改状态或预留后续接纳资格, 也不是远端查询 SDK.

## Agent 与 Authority 的联系

Agent Loop 位于执行它的 Worker, 通过宿主提供的 Client Interface 联系 Authority. Client 不要求独立进程. 外部 UI, Loop, Codemode 的调用者使用相同提交与查询语义, 内部调用额外携带合法执行来源.

这些能力通过可复用的库接口提供, Node/Console 与其他应用均调用该接口. 库负责提交关联, 去重, 既定重投规则, 查询/通知合并与断线补齐; 消息 Adapter 负责连接, 订阅与编码. 应用选择目标与配置, 发起请求并消费结构化观察, 无需自行组装底层信封或重新实现交付状态机.

应用通过公开构造与绑定接口为 Client 指定本地交付或消息 Adapter, 以及所需的身份/路由配置. 可以传入已建立的共享连接, 连接由显式的基础设施初始化调用创建; 创建 Client 不自动启动 Authority, Worker, 数据库或全局观测性. 通用 Client 不硬编码 Zenoh, 不要求应用通过统一 Node 构造器才能使用. Client 仅释放自己的订阅与资源使用关系, 共享基础设施的关闭遵守应用声明的所有权.

```text
Worker A: Agent Loop
  -> Authority X: 提交 Tool 请求, 原 SubmissionKey K
  <- 接纳映射: K -> Operation O / Attempt T

Authority X -> Worker B: 授权 T
Worker B -> Authority X: Report R
Authority X: 校验并保留 R
Authority X -> Worker A: 已保留的执行观察
```

工具 Worker 不需要知道 Agent 当前连接地址. 它向授权它的 Authority 交付 Report, Agent 从 Authority 读取已验证事实. 通知可携带完整 Report 或 ReportRef, 不把原 Worker-to-Authority Report 的绑定改写成发给 Agent 的执行授权.

## 最小逻辑 Interface

以下伪 IDL 仅借鉴 WIT/Protobuf 的契约表达, 不选择其编码或代码生成工具:

```text
interface execution {
  submit(authority_id, submission) -> Admission
  query_submission(authority_id, submission_key) -> SubmissionObservation
  query_operation(authority_id, operation_id) -> OperationObservation
  query_report(authority_id, report_id) -> Report
  observe(authority_id, selection) -> stream<ExecutionNotice>
}
```

`selection` 可以在一个共享订阅上选择多项执行, 不要求每次 Operation 建立一个网络订阅. 名称与具体返回类型在实现时收敛, 这些逻辑能力不对应固定数量的网络往返.

| 契约 | 必须保留的语义 |
| --- | --- |
| submit | 原 SubmissionKey 与不可变请求同送; 同 Authority 同键同定义返回原接纳映射; 内容冲突拒绝 |
| query_submission | 接纳回执丢失时找回原 Operation/Attempt; 查不到与确定未接纳不是同义词 |
| query_operation | 返回 Authority 已知事实及 Report 引用; 不从心跳或本地超时捏造远端执行状态 |
| query_report | 返回完整不可变证据, 包括未选为代表结果的附加 Report; 仍检查查询权限 |
| observe | 及时传递已保留事实, 允许重复; 不承诺订阅本身可跨重启恢复或恰好一次通知 |

`Unavailable`, `NotFound`, `Forbidden`, 内容冲突和已过保留范围必须能区分. 网络不可达不是 NotFound, NotFound 也不能在去重记录可能丢失的场景中证明原请求从未执行. 禁止使用新的键自动填补查询失败.

接纳回执与实际派发可以在接纳事务成功后独立进行. 不需要等待 Client 收到回执才派发工具. Report 先到时先校验关联并保留, 同样可以补齐提交映射; 后来的回执不能回退结果状态.

## 稳定身份, 路由与授权

持久引用使用稳定 AuthorityId, 运行会话使用当前可访问端点和运行实例. Client Adapter 负责根据配置解析, 领域数据不保存临时连接对象. 首版可以采用静态映射, 不要求服务发现框架.

Authority 实体标识, 运行实例标识, endpoint 与认证主体分别处理. 相同 UUID 不证明持有原账本或有权接管. 重启时恢复旧记录, 路由新实例和接受旧 Report 的规则见 [持久化与恢复](persistence-recovery.md).

来源 AgentId, 父 ExecutionRef, LLM 自报的 Tool 名称和 Zenoh 路由均不自动授权. 至少区分读取 Agent/执行数据, 提交 capability, 使用外部资源和推进 Agent 的权利. 首版使用静态可信部署与明确允许列表, 不引入自动信任注册. 公网或不可信节点接入前须落实实际认证, 不能把原型绑定校验描述为安全认证.

## 结果订阅与恢复补齐

正常观察流程:

1. 建立可接收通知的共享通道, 确认注册成功, 缓冲期间到达的通知.
2. 查询所跟踪执行的当前状态与已有 Report.
3. 合并快照与通知, 按执行绑定和 ReportId 去重.
4. 选定并保存业务采用/消费位置, 再推进 Loop.

观察者注册成功是应用层条件, 不等于仅发现一个传输订阅者. 结果可能先于订阅完成, 所以必须查询已有事实. 查询响应与通知可以倒序或重复, 合并不能使已知事实回退.

断线后按持久引用重新查询, 然后恢复观察. 即使连接未报错, 尽力通知也可能丢失; 尚未满足的等待条件应在截止点或受限周期内对账查询. 没有持久通知游标或可靠注册协议时, 不能仅靠一次订阅加查询宣称永不漏结果. 首版可以依靠保留事实与重复查询收敛, 不要求另建可靠消息日志.

Report 收录不等于 Agent 已处理. Agent 对已处理结果去重, 不因额外 Report 自动再执行另一套工具. 订阅对象和 future 不序列化; 执行引用与消费位置持久化.

## UI 接收 Agent 回答

Agent 层提供已发布回答的查询与通知. UI 从 Agent 的保存记录取得回答, 可在异步工具和后续 LLM Operation 尚未结束时展示已有内容; 整个 Loop 的最终 Report 表达该 Agent 执行的结束. 这两种观察复用现有消息 Adapter, 不需要额外执行原语.

回答记录保存稳定位置, 内容或可查询的内容引用及来源关联. UI 合并查询与通知时按该位置去重, 遗漏通知或重新连接后查询补齐. 同一 LLM Report 重复处理不创建第二条发布记录; 查询回答不要求原 Loop 仍在运行.

查询/通知合并, 去重和补齐由 Agent 输出观察的库接口承载, UI 消费合并后的回答并负责展示. 不要求每个 Console 或 UI 分别实现这些规则.

持久模式先提交回答记录再通知 UI, 通知丢失不丢失已发布内容. 易失模式按其明确约定保留内存记录. 首版传递完整回答, token 流式片段不属于本次接口承诺. 输出记录与发布选择归 Agent, 原始 LLM Report 仍归执行账本, 具体方法名和编码在实现时确定.

## 传输和延迟

同进程直接类型化调用或通道交付. 跨进程采用消息 Adapter, 当前主要使用 Zenoh, 不强制 Broker/Router. 路由可达性必须实际建立, 连接某个 peer 不意味着任意节点天然互通.

复用连接与订阅, 通知优先携带已有完整 Report, 查询用于补齐和审阅. 独立调用并行, 可批量传输或提交事务, 但保持每个调用独立身份. 不为每个字段或状态变化增加网络往返.

新增记录前检查实现所需的字节/队列边界, 满时背压或明确拒绝, 不通过删除去重记录重新获得执行资格. 这些是 Adapter 资源管理, 不引入统一 Operation 容量原语.

确认语义由运行模式明确约定: 易失模式只确认内存接纳, 持久模式在数据库提交后确认. 两者不能在 Client 不知情时静默降级. 具体协商字段属于后续协议设计.
