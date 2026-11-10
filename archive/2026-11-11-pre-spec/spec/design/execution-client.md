# 执行 Client 与结果观察

状态: Interface 的语义契约, 2026-11-10. 通用 Authority 已实现按 SubmissionKey 的内存接纳, beamlet 已提供本地 LocalClient 提交及查询, 以及不依赖编码和连接实现的 remote::Client 提交, 只读查询与结果观察. Console 已通过库调用远端 Hello 提交, 查询和通知/查询合并. [回执契约](receipts.md)描述当前易失原型.

当前库入口为 `Authority::submit_keyed(key, task, allocate)`: 定义按类型 P 的 Eq 比较, 传输信封和临时回复地址由 Adapter 处理. 只有新键通过记录容量检查后才调用 allocate 生成 OperationId/AttemptId; 同键同定义返回原授权, 冲突或容量不足不生成身份. 已用 ID 被拒绝, 不把新键关联到已有执行. `check_submission(key, &task)` 返回已接纳执行的当前观察或定义冲突, 未知键返回空值; 它不修改状态或预留后续接纳资格, 也不是远端查询 SDK.

## 当前本地 Client

`LocalClient::new(&mut authority, &mut worker, allocate)` 借用应用显式提供的角色和 ID 生成函数; `LocalExecution::client(allocate)` 是相同绑定的便捷入口. 两种方式不创建角色或基础设施. Client 只在首次接纳后将原授权交付 Worker 入队, 释放借用后由宿主继续驱动 Worker 并向 Authority 收录 Report. 重新绑定 Client 可以查询已有结果; 创建或丢弃 Client 不取消任务或丢弃角色状态.

当前方法及返回值:

| 方法 | 返回的事实 |
| --- | --- |
| `submit(key, task)` | SubmissionReceipt: 原提交键, New/Duplicate 接纳结论, 当前 OperationObservation |
| `query_submission(key)` | 原执行的 OperationObservation, 无需重发任务定义或知道 OperationId |
| `query_operation(id)` | 当前 OperationObservation, 含原执行绑定, 任务副本, 局部阶段, 等待观察及采用的 ReportId |
| `query_report(id)` | ReportObservation, 含完整 Report 副本及原 Worker 到 Authority 的身份方向; 也可查询附加证据 |

查询返回独立快照, 不创建执行, 不分配 ID, 不修改等待状态. 未知键, Operation 或 Report 分别返回对应 NotFound; 同键不同内容是 SubmissionConflict. Worker 拒绝交付时, `LocalClientError::Dispatch` 携带已接纳的 ExecutionBinding 与交付错误; 原提交键仍能查到 Accepted, 没有合成失败 Report. 再次提交同键只返回原接纳事实, 即使后来队列空闲也不自动重新交付.

ExecutionBinding 当前包含 `authority_instance_id`, `worker_instance_id`, OperationId 和 AttemptId. 前两项是运行实例身份, 不冒充跨重启的稳定 AuthorityId. 当前绑定只访问可信同进程内存账本; NotFound 仅表示该账本未保留记录, 不能证明重启前从未执行. 本地同步查询不引入网络不可达或权限错误占位项; 远端查询单独区分传输失败与未找到.

Node/Console 的共享 `--demo` 已通过该 Client 提交 100 个任务, 由宿主驱动执行, 再按原键找回执行并查询报告. 固定任务协议仍用于独立进程演示, 没有混入本地 Client 接口.

## 当前远端提交 Client

`beamlet::remote::Client::new(transport, codec, expected_authority)` 接受应用提供的专用回复通道, 编码 Adapter 与可选目标实例. Transport trait 只负责有界字节发送和可取消接收, Codec trait 只负责完整提交编码与严格回复解码. 通用库拥有重投预算, 提交键及执行关联校验和结果等待, 不依赖 Zenoh 或固定 Hello 类型.

Zenoh 的 `ClientTransport::bind(&mut transport, target)` 在已有会话上声明目标发送端, 借用已有回复订阅, 不另建连接. 独占借用防止多个 Client 竞争同一回复队列; 应用可事先通过 Transport::subscribe 建立共用 Session 的其他独立订阅. 释放 Client 后原连接仍归应用. Console 选择连接, 路由及可选 Authority, 使用测试 Adapter 的 HelloClientCodec, 调用 Client 后展示结构化结果.

`submit(key, &task)` 编码一次, 最多三次发送相同字节, 每次匹配及发送最多 500ms, 两次发送间隔 1s, 总观察期限 10s. Report 或 Pending 可以先于独立接纳回执到达, 校验成功即可绑定原 Authority/Operation/Attempt 并停止重投. 接纳映射不含 Worker 身份, 不为缺失字段构造虚假值; 完整 ReportObservation 保留原 Worker 到 Authority 的方向. 错键, 错实例, 冲突映射及非法证据不确认接纳; 已确认映射不被后来的无映射拒绝推翻.

结果 `Submission` 包含原键, Binding 和 Outcome: Pending, 完整 Report, 或已关联请求的 SubmissionError. 业务失败和执行前拒绝保留在 Report 中. 编码失败, 未取得映射的提交拒绝, 接收连接错误及总期限到达分别返回 ClientError; 断线和超时携带原键, 已知映射及已发送次数, 不声称远端执行已停止. 取消等待 future 停止本地驱动, 不产生后台重投; 显式再次调用 submit 才开始新的调用预算.

提交和单次查询返回第一份匹配观察. Console 在收到该观察后展示关联及结果, 超时或断线也可展示已有映射. 持续结果观察由下述 observe 方法承载; 同一回复队列仍顺序处理调用.

## 当前远端只读查询

`beamlet::query` 定义 Request, Selection, Reply 和独立的 Codec trait. Request 包含应用为每次查询生成的 `query::Id`, 明确的 Authority 运行实例, 以及原提交键/OperationId/ReportId 三选一. 查询 ID 仅关联消息, 不是执行身份; 同一回复队列及其重建后必须保持唯一. 重新查询同一选择也使用新的查询 ID, 防止迟到的旧快照满足本次查询.

`Authority::answer_query(&self, request)` 只读访问账本, 与 LocalClient 使用相同快照投影. 查询返回完整 OperationObservation 或 ReportObservation, 包含原 Worker 绑定和附加证据; 未保留记录返回 NotFound. 目标不是本实例返回 WrongAuthority, 不代替另一账本回答 NotFound. 接口不接收 Worker 或执行 ID 分配器, 不接纳/派发任务, 不改变等待状态或交付预算.

remote::Client 在编码器实现 `query::Codec` 时提供以下方法:

| 方法 | 返回值 |
| --- | --- |
| `query_submission(query_id, key)` | OperationObservation |
| `query_operation(query_id, id)` | OperationObservation |
| `query_report(query_id, id)` | ReportObservation |
| `query(query_id, selection)` | 带类型的 Operation/Report 观察 |

查询必须在构造 Client 时提供 `Some(authority_instance_id)`, 未指定时在任何 I/O 前返回 AuthorityRequired. 从发送条件等待到匹配回复共用 10s 总期限, 只发送一次, 无自动重投. 回复必须匹配本次查询 ID, Authority, 选择及执行绑定, 并通过状态/证据校验; 非法及无关回复不延长期限. 编码失败, Transport, TimedOut 与匹配的 NotFound 分别返回 `query::ClientError`. 业务失败的 Report 仍是成功查询得到的证据.

应用显式为查询绑定目标路由; Hello Adapter 使用 `beamlet/v0/authority/{instance}/query/{console}`. ClientTransport 和 HelloClientCodec 同时供提交与查询使用. Node 为每个 Authority 订阅查询入口, 将请求交给测试 Adapter 的 `query::respond(&authority, bytes, limits)`, 排队发送返回字节; 该入口在仍运行的 Draining 期间也可读取, 发送队列满只丢弃本次回复, 不修改记录. 查询路由与旧提交接入约定独立, 默认本地组合 Node 同样提供查询.

Console 支持 `--authority UUID --query-submission UUID`, `--query-operation UUID` 或 `--query-report UUID`; 查询选项互斥, 不与提交参数混用. 每次连接使用独立回复地址, 每次查询生成独立查询 ID. 原 Console 退出后, 新 Console 可以只带原键和原 Authority 找回全部执行关联, 再按 ReportId 读取证据. 这要求原 Authority 仍保留内存账本; 重启丢失记录, 旧实例不可达或查询失败都不能自动转为新提交. 当前没有认证/权限服务或跨重启持久恢复.

## 当前结果观察与补齐

`observe::State::new(authority, operation, max_reports)` 由应用持有, 跟踪一个原执行. 它保存单调的 Operation 快照及按 ReportId 去重的完整证据; 不包含连接, future 或执行权. `merge_operation`, `merge_report`, `merge_notice` 可直接用于类型化调用. 较旧的 Accepted/Dispatched 或 Waiting 不覆盖已知的 Reported/TimedOut; 任务, 四项执行绑定, 已采用 ReportId 和同 ID 证据内容不能改变. 违反约束或报告容量不足时返回错误并保持原状态, 不删除去重记录.

`observe::Notice` 携带执行快照和一份完整已收录 Report. `Authority::report_notice(report_id)` 从账本只读构造它. 报告可以是附加证据, 不能仅因它先到就认定它被采用; 采用决定来自快照. 两部分先联合校验再原子合并. Change 标明新报告 ID 与快照是否增加事实, 重复消息不产生重复变化. `take_adopted_report` 仅在采用决定和正文均已知时返回报告, 同一个 State 至多返回一次; 这不是 Agent 的持久消费事务.

`remote::Client::observe(&mut state, &notices, budget, allocate_query_id)` 接受应用已建立的独立通知通道, 使用原查询通道对账. 编码器实现 `observe::Codec`, 通知通道实现 `observe::Notifications`. 默认总等待 10 秒, 单次查询含发送就绪最多 500ms, 未满足结果条件时每隔 1 秒再次查询; 所有期限由 Budget 显式配置, 通知和旧回复均不刷新总期限. 开始调用立即查询, 得到采用的 ReportId 后立即补齐缺失正文; 等待查询期间同时合并通知, 完整通知可以直接满足等待. 查询每次使用新的关联 ID, 只读方法不创建新提交键/执行或重投授权.

返回 Some(ReportObservation) 表示本 State 首次交付采用结果, None 表示此前已交付. 业务失败的 Report 同样是有效观察结果. 明确的 NotFound, 传输故障, 通知通道错误, 冲突/容量错误以及总观察超时分别返回结构化错误. 单次查询超时可在剩余总预算内继续对账. 取消 future 停止本次 I/O, 已合并 State 保留; 应用重建连接与订阅后把同一 State 绑定新 Client, 会查询尚缺的原事实. 不自动解析新 Authority 实例或隐式连接基础设施. 进程重启后恢复消费位置仍需后续持久化实现.

Hello Adapter 的共享通知主题是 `beamlet/v0/authority/{instance}/observations`. Node 仅在成功收录后排队发送库生成的通知, 与其他发送共用有界队列, 每次最多等待 500ms; 队列满或无人接收时可以遗漏通知, 保留事实仍可查询. 本批采用尽力广播和周期查询, 不声称本地订阅建立就是 Authority 确认注册, 也不依赖一次订阅加一次查询保证结果不遗漏. 通知通道必须在 observe 调用前由应用建立, 查询回复使用另一队列; 应用可让两者共享已有 Zenoh Session.

Console 的 `--authority UUID --observe-operation UUID` 在库中等待已采用结果并展示一次, 与三种单次查询/提交选项互斥. `--subscribe` 仍独立表示角色生命周期通知. 当前驱动一次跟踪一个 Operation, 一个通知队列只由一个驱动消费; 并行观察可显式建立共享 Session 的独立订阅. 附加报告可合并和查询, 但本方法的完成条件是已采用报告, 不承诺枚举后来所有附加证据.

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
