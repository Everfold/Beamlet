# 能力交付契约

状态: 已实现 Worker pool、会话登记与定向交付. node / console 使用同一 v3 协议; 不兼容旧纯文本网络入口. 启动和独立角色配置见[Node 角色装配](node-composition.md).

## 1. 职责与边界

领域原语仍只有 [Operation、Attempt、Report](kernel.md). Authority 创建 Operation、签发 Attempt、接受 Report 并关闭 Operation; Worker 执行收到的 Attempt 并创建 Report. Worker 不持有 ledger, 登记表不持有 Operation. 共享 Worker 不表示共享或接管其他 Authority 的 Operation.

Operation 原始请求为 `beamlet_api::Task<P> { capability, payload }`, 当前文本示例的 P 为 String; v3 线上消息仍使用 `Task { capability, text }`, 由 protocol 显式转换. pool、目标实例与请求关联信息是应用层路由上下文, 不改变 kernel / ledger 的状态转换. [beamlet-api](../crates/beamlet-api/src/lib.rs) 共享角色身份、pool、能力标识、状态快照、泛型业务数据及一次性回复入口; [beamlet-protocol](../crates/beamlet-protocol/src/lib.rs) 定义 v3 消息结构、版本与 subject 编码, 不管理连接、选择策略或生命周期.

node 的 ingress 接收本地终端和网络提交, 返回结构化请求与一次性回复入口. delivery 选择并向 Worker 发送一个调用, 返回完整 Report 或交付错误. channel 实现只用于测试, 不是生产回退路径.

## 2. 身份、pool 与能力

每个角色使用持久 `role_id` 和本次角色运行独立生成的 `session_id`. Node 可同时承载多个 Authority / Worker; `node_id` 只用于承载进程的日志, 不参与路由. label 只用于展示. 同一 role_id 的不同会话独立登记、独立路由; 不按 UUID 大小决定新旧. 复制角色配置同时运行会展示多个会话, 不承诺单角色单活.

每个 Worker 角色属于一个 `pool`; 每个 Authority 角色加入非空 `pools` 集合. 角色引用一个命名基础设施, 同一个 Node 可共享或配置多个 NATS 连接; 独立 NATS 上的同名 pool 彼此不可见. Console 的 `pools` 是发现范围. 名称为 1..=64 字节 ASCII 字母、数字、`-`、`_`, 按大小写敏感的精确值去重. 不修剪或改写配置中的名称; 去重不重写文件. Authority 之间无需共享 Worker 表: 同池公告由各 Authority 分别接收, 视图允许短暂不同.

Worker 配置的 `capabilities` 同时决定公告内容和执行许可, 空列表或未知版本使启动失败. 目前内置:

- `text.reverse/v1`: 按 Unicode 标量值倒写, 保留空白, 不保持组合字形或复合 emoji.
- `text.uppercase/v1`: Rust Unicode 大写转换, 保留空白, 允许 `ß → SS` 这样的字符展开.

能力精确匹配, 没有隐式版本兼容或组合条件. 能力执行返回 Future, 可以使用异步 I/O; 现有文本能力将 CPU 计算放在阻塞任务中, 不阻塞异步心跳循环. 等待中的 Future 仍计入 Worker 并发上限.

## 3. Worker 登记与存活

每个 Authority 持有内存登记表, 键为 `(pool, role_id, session_id)`. 完整公告含 label、能力集合、`ready/draining/offline` 状态及会话内递增的 `sequence`. 同序号或倒序消息不更新状态, 也不续期. 心跳默认每 5 秒一次, Worker 可通过 `heartbeat_interval_secs` 修改.

Worker 在任务订阅刷新后公告 ready. Authority 启动时主动探测, Worker 收到探测即发布完整快照; 即使丢失上线消息, 后续完整心跳也能登记. 多 pool 订阅和公告均不使用共享队列组.

Authority 使用本地单调接收时间判断存活. `worker_timeout_secs` 默认 15 秒; 应覆盖至少约三个心跳周期. 查询和选择即时判断是否过期, 不依赖清理时机. draining、offline、过期记录均不可接受新任务. 后续更大序号的 ready 可以恢复候选状态. 下线只影响指定会话.

每秒检查失效并记录日志; 记录自最后一次有效公告起最多保留 `4 × worker_timeout_secs`, 随后清理. 保留期间记住序号, 防止旧 ready 立即复活已下线记录. 超过保留期后不承诺检测任意长期重放. 登记不持久化, Authority 重启后重新发现.

NATS 断开时所有现有记录立即停止用于选择; 重连后的连接世代要求新心跳重新确认, 不直接复用原存活时间. Worker 断线期间跳过周期公告, 重连刷新订阅后立即公告, 不补发错过的周期. 正常退出先停止订阅、公告 draining, 排空已接收调用后尽力公告 offline. 强制退出依靠期限判断.

心跳和节点查询处理独立于 Authority 在途名额和 Worker 执行. 正常重复心跳不输出 INFO; 上线、状态变更、失联和恢复输出日志.

## 4. 版本、发现与定向路由

JSON 外层统一为 `{ "version": 3, "body": ... }`. 未知版本、格式错误、字段错误不执行业务. 配置仍使用 RON.

| subject | 用途 |
| --- | --- |
| `beamlet.v3.<pool>.authority.discover` | 发现该 pool 内的 Authority, 每个 Authority 回复自身信息 |
| `beamlet.v3.<pool>.authority.<role_id>.<session_id>.requests` | 指定 Authority 的 Worker 查询或创建请求 |
| `beamlet.v3.<pool>.worker.presence` | 广播完整状态快照 |
| `beamlet.v3.<pool>.worker.probe` | 请求 Worker 立即发布快照 |
| `beamlet.v3.<pool>.worker.<role_id>.<session_id>.calls` | 指定 Worker 会话的调用 |

Console 对配置的 pools 发出发现查询, 在 `discovery_window_ms` 指定的窗口内收集多份回复 (默认 1000 毫秒). 按 Authority 实例去重并保留 pool 信息; 发现不是只取首个回复的普通 request. 后续 Worker 查询和创建请求均发到选定 Authority 会话.

Worker 选择为 `Any` 或 `Exact(instance)`. 两者都要求明确 pool 和能力. Authority 创建 Operation 前检查租约、状态、能力及会话. Any 选定一个实例后走与 Exact 相同的定向路径. 不承诺公平、轮询或负载均衡. Exact 失效时拒绝, 不回退到其他会话.

一次调用只发布到一个 Worker 实例主题. Worker 校验内部 pool、目标身份、能力及调用关联非空. 有效调用只向原回复地址发送 Report. Authority 对照签发时的目标 pool、Worker 会话、Operation 和 Attempt 校验回复; 不以当前租约是否仍存活决定是否接受 Report.

身份和 subject 用于路由关联, 不是身份认证. 部署方需保证实例主题仅由相应 Worker 订阅; 本协议不提供恶意订阅防护或恰好一次执行保证.

## 5. 容量和失败语义

每个 Authority 角色通过 `max_concurrent_tasks` 配置同时交付并等待报告的上限, 默认为 `16`, `0` 表示不限. 所有 pools 与目标 Worker 共用该角色的上限, 其他 Authority 独立计数. 满载时暂停业务接收, 完成或失败时释放名额; 失败释放名额不会改变仍等待原 Report 的 Operation 状态. ingress 根据该 Authority 的 `input_capacity` 缓冲创建请求 (默认 16), 与角色并发设置独立. 队列已满的请求明确拒绝, 控制查询仍可处理. Authority 的结果与拒绝回复独立异步发送, 按该角色 `max_concurrent_replies` 限制待发送回复 (默认 16); 满载时暂停业务接收和报告收取, 恢复发送后继续推进. 每个 Worker 角色通过 `max_concurrent_tasks` 配置并发上限, 首次生成配置默认为 `1`, `0` 表示 Worker 不限制并发. 从取出调用到执行和报告发送完成或失败期间占用一个名额, 满载时暂停取出新调用. 一个角色实例订阅使用其 NATS 实例 `subscription_capacity` 指定的接收缓冲 (默认 16 条), 多个 Authority 共享该角色的并发容量; 同一 Node 内其他 Worker 有独立名额和队列. `0` 不改变线程池、硬件、Authority 在途数和接收缓冲的独立限制. ready 不预留执行槽位. NATS 缓冲溢出无法证明交付结果时最终体现为未知/超时, 不伪称明确未交付.

| 时机 / 结果 | 行为 |
| --- | --- |
| 创建前校验失败 | `Rejected`, 没有 Operation / Attempt 身份 |
| 已签发, 无接收者或明确拒收 | `Unresolved`, Operation 保持 AwaitingReport |
| 已签发, 等待报告超时或报告关联错误 | `Unresolved`, Operation 保持 AwaitingReport |
| 有效完整 Report | Authority 接受并关闭 Operation, 返回 `Completed` 及全部关联身份、执行 Worker 和结果 |

Report 身份按接收顺序分配, 有效报告按完成顺序返回, 无需等待较早的慢任务. 执行拒绝和回复失败释放 Worker 名额, 不重试. 退出时停止新接收, 按配置并发排空队列并等待运行中任务, 受 Node 退出期限约束.

Report 等待期限由 Authority 的 `report_timeout_secs` 指定 (默认 3 秒); Console 请求期限由 `request_timeout_secs` 指定 (默认 10 秒), 包含正常排队和回复开销. 校验与发送之间仍可能失联. 无论拒收、断线或超时, 均不取消已签发 Attempt、不换人重发、不伪造 Report、不强制关闭 Operation. 超时不取消 Worker 执行; 本版 request 超时后可能丢失迟到 Report, 不提供持续监听或恢复.

`request_id` 只用于请求/回复关联, 不提供重复创建幂等. Console 支持多个在途请求, 每次提交保存目标快照, 按完成顺序展示结果并标识 request_id. 发现和查询也独立等待, 迟到结果不能覆盖较新的选择或查询. Console 按 `max_in_flight_requests` 限制网络请求数 (默认 16), 满载的新请求明确拒绝; `/quit` / EOF 排空在途请求, Ctrl+C 停止本地等待. Console 超时不自动重发. 任何“已提交”日志仅说明传输接受提交, 不证明接收者已执行. 重试、持久化恢复及跨 Authority 接管不属于当前范围.

Node 将退出信号广播给所有角色. Authority 停止新接收、丢弃尚未签发的排队请求, 等待已签发交付和回复发送结束并刷新回复缓冲; Worker 排空各自队列. Node 的排空期限和失败处理见[角色装配](node-composition.md). 本地 stdin 统一读取并选择一个 Authority, EOF 不停止网络服务. 内存状态随进程退出丢失.

## 6. 验证

普通 `cargo test --workspace` 不需要 NATS. [登记测试](../apps/beamlet-node/src/registry.rs)使用可控时间验证期限、序号、会话隔离、重连与清理; [channel 交付测试](../apps/beamlet-node/src/delivery/tests.rs)验证定向交付、无回退、容量与报告关联; [入口测试](../apps/beamlet-node/src/ingress/tests.rs)验证并发、满载、退出与失败语义. 共享协议测试验证版本和主题隔离.

显式运行 `cargo test -p beamlet-node --test composition -- --ignored --nocapture` 可进行真实 NATS 组合验收. 测试要求 Unix 和 PATH 中的 `nats-server`, 使用临时配置、独立端口和子进程并自动回收. 覆盖一个进程内多个同类型角色、多连接共享/隔离、精确角色定向、同角色不同会话、跨 Node 运行、正常退出、重连和配置不变. 普通 Cargo 测试默认忽略该验收, 不依赖服务端. 这些检查不构成任意网络故障下的恰好一次证明.

Console 的真实 NATS 并发验收使用 `cargo test -p beamlet-console --test concurrency -- --ignored --nocapture`, 同样需要 PATH 中的 `nats-server`. 普通 Console 测试通过可控 Future 验证多个请求、迟到回复隔离、容量限制、EOF / 退出排空和中止时释放 Future.
