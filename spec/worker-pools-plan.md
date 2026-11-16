# Worker 能力、Pool 与定向交付开发计划

状态: 待实施. 本文记录已讨论的目标、建议的实现细节、开发顺序与验收要求. 当前行为仍以 [交付契约](delivery.md) 和 [内核规范](kernel.md) 为准; 各阶段完成时同步更新交付契约.

**1. 目标与已确定的约定**

- Worker 支持按配置启用内置能力. 第二种示范能力采用 `text.uppercase/v1`, 与现有 `text.reverse/v1` 一样接受并返回 UTF-8 文本.
- Worker 属于一个 pool; Authority 可以加入多个 pool. 同池内多个 Authority 可以使用同一个 Worker.
- Authority 的 pool 名称按大小写敏感的精确值去重. 重复配置只产生一次订阅; 空名称报配置错误.
- Authority 各自维护内存 Worker 登记表, 用于查询、能力过滤与目标校验. Operation 仍由创建它的 Authority 管理.
- Worker 就绪时立即公告, 默认每 5 秒发送完整心跳, 周期可配置. 正常退出公告下线, 异常退出依靠超时判断.
- Console 可以手动选择 Authority、pool、能力和 Worker. 目标锁定具体 `node_id + session_id`, label 只用于展示.
- 配置由 confy 在文件不存在时创建, 保留可执行文件旁的文件位置及 node 的 `--config/-c`. 不做旧配置迁移、自动补字段或已有文件重写; 去重只影响内存.
- 复用 Operation / Attempt / Report 及现有 kernel 转换. 登记去重不意味着执行去重, 共享 Worker 不意味着共享或接管 Operation.

**2. 配置与业务数据**

以下是新增字段的设计, 具体 RON 布局在配置阶段与现有 `role` 字段一并落实; 共用身份、连接和颜色字段继续沿用现有实现.

| 使用方 | 字段 | 默认值及校验 |
| --- | --- | --- |
| Authority | `pools` | `["default"]`; 集合非空, 用 `BTreeSet` 精确去重并保持稳定遍历顺序 |
| Authority | `worker_timeout_secs` | `15`; 必须大于 0, 运维配置应覆盖至少约三个 Worker 心跳间隔 |
| Worker | `pool` | `"default"`; 单个有效名称 |
| Worker | `capabilities` | `["text.reverse/v1"]`; 非空、精确匹配内置版本, 重复项只启用一次 |
| Worker | `heartbeat_interval_secs` | `5`; 必须大于 0 |
| Console | `pools` | 建议新增, 默认 `["default"]`, 作为 Authority 发现范围; 去重规则相同 |

pool 名称建议限定为非空 ASCII 字母、数字、`-` 和 `_`, 不自动修改大小写或修剪输入. 这样可直接成为单个路由主题片段, 避免点号或通配符改变分组. 这是待实施的输入约束, 不是已有配置要求.

内置能力列表统一提供标识、简短说明和执行入口. 启用列表同时决定 Worker 的订阅、状态公告与实际执行许可. 配置包含未知能力或未知版本时启动失败, 不静默忽略. `text.uppercase/v1` 按 Rust Unicode 大写转换规则处理, 允许字符展开, 保留空白; 例如 `Beamlet → BEAMLET`, `ß → SS`.

Operation 的原始请求改为具名业务数据, 至少包含 `capability` 与 `text`. 目标 pool、Authority / Worker 实例以及请求关联信息属于应用层提交与交付上下文, 不加入 kernel 的状态转换规则.

**3. 登记与心跳规则**

Worker 登记按 pool 分组, 组内键为 `(node_id, session_id)`. 每条记录保存 label、启用能力、公告状态和本地最后接收时间. 相同键的公告更新原记录; 不重复插入 Worker. 同 node_id 的不同 session 独立展示和校验, 不根据 UUID 大小猜测哪个会话更新.

公告使用完整快照: `pool`、实例身份、label、能力集合、`ready/draining/offline` 状态, 以及建议增加的会话内递增 `sequence`. 相同序号保持幂等, 更旧序号不覆盖状态或续期. 显式下线后保留有限时间的序号记录, 避免旧 ready 公告立即恢复该会话.

| 事件 | Authority 的处理 |
| --- | --- |
| 新实例 ready 公告 | 在已加入的 pool 内登记并记录本地接收时间 |
| 更新的完整心跳 | 刷新记录和接收时间; 丢失最初上线消息时也能登记 |
| draining | 从新任务候选中排除, 保留身份与诊断信息 |
| offline | 从新任务候选中排除; 只影响该 pool 中的指定会话 |
| 超过有效期 | 标记失联; 这是 Authority 的观察结果, 不等于证明进程停止 |
| 更新的 ready 心跳恢复 | 重新检查身份、pool 与序号后恢复可选状态 |
| Authority 断开 NATS | 暂停使用已有记录进行新交付; 重连后以新公告重新确认 |

使用 Authority 本地单调时钟计算有效期, 不依赖 Worker 墙上时钟. 查询和实际选择时都检查期限, 不依赖定时清理是否已经运行. 失联、下线记录需要有界保留与清理, 避免反复重启导致登记表无限增长.

Worker 完成任务订阅及刷新后才能公告 ready. 心跳发送、Authority 接收与查询处理独立于业务执行及在途交付名额; 长任务和满载不能阻止心跳处理. 重连后恢复订阅并立即发布新快照, 不补发一串历史心跳. 退出时停止接收新任务, 公告 draining, 排空已接收工作后尽力公告 offline 并刷新缓冲.

每个 pool 的公告由所有相关 Authority 普通订阅, 不使用共享队列组. 各 Authority 的登记视图允许短暂不同. Authority 晚启动或重启时通过后续完整心跳重建登记, 可增加一次主动探测缩短首次等待, 不要求持久化节点表.

**4. 协议与路由**

Console 与 node 共同使用的消息定义集中维护. 建议仅提取一个小型 `beamlet-protocol` crate, 放置消息结构、版本及统一主题编码; 不放连接管理、执行策略或生命周期管理. node 内部的登记、能力执行、ingress 和 delivery 继续按职责组织.

| 消息 | 必要信息与用途 |
| --- | --- |
| Authority 发现查询 / 回复 | 请求关联标识; Authority 实例、label、已加入 pools、协议版本 |
| Worker 查询 / 回复 | 指定 Authority 实例、pool/能力过滤; Worker 实例、能力、状态、距上次心跳时长 |
| Worker 状态公告 | 完整身份与能力快照, 供登记及续期 |
| 创建请求 | `request_id`、Authority 目标、pool、Worker 选择、能力与文本 |
| 执行调用 | 原有 Operation / Attempt 身份与业务数据, 加上已选定的目标实例和回复方上下文 |
| 执行回复 | 完整 Report 或明确的交付拒绝; 携带目标关联信息以便校验 |
| Console 最终回复 | 请求关联信息; 创建前拒绝、已完成或等待报告未解决; 已创建时返回 Operation / Attempt 等身份 |

Worker 选择表达为 `Any` 或 `Exact(node_id, session_id)`. Any 也限定在明确的 pool 和能力内; Authority 从登记表选择一个实例后, 走同一条定向交付路径. Exact 校验失败直接返回, 不回退到其他 Worker.

路由主题统一版本并划分用途, 建议形态如下; 最终格式在协议阶段固定并测试:

```text
beamlet.v2.<pool>.authority.discover
beamlet.v2.<pool>.authority.<node_id>.<session_id>.requests
beamlet.v2.<pool>.worker.presence
beamlet.v2.<pool>.worker.<node_id>.<session_id>.calls
```

Authority 为每个去重后的 pool 注册发现和请求入口. Console 在配置的 pools 内收集多份发现回复, 按 Authority 实例去重, 同时保留 pool 信息. 查询 Worker 时使用选定 Authority 的请求入口. 发现使用有限收集窗口和普通订阅, 不使用只取首个回复的调用方式收集全部节点.

一次任务只发布到一个实例主题; 不同时发布到公共能力主题和实例主题. Worker 校验消息里的 pool、会话、能力与自身配置一致, 不执行跨池或旧会话调用. 最终回复发送给原请求方, 不广播 Report. 主题和消息身份用于路由关联, 不把它们解释成身份认证.

`request_id` 第一版只承担关联作用, 不承诺重复提交幂等; Console 超时不自动重发创建请求. 采用新协议时同时更新 node 和 console, 不添加旧纯文本网络协议的兼容分支.

**5. 任务容量和失败语义**

Authority 在创建 Operation / Attempt 前检查目标 pool、Worker 会话、有效期与启用能力. 创建前拒绝不产生新的生命周期对象. 检查通过与实际发送之间仍存在竞态, 不能把预检当成交付保证.

每个 Worker 的执行容量与有界队列由该 Worker 全局管理, 所有 Authority 共享同一限额. ready 表示允许尝试提交, 不表示预留了执行槽位. 第一版保持现有串行执行和有界缓冲, 不扩展成并行调度框架. 接收层能够明确拒绝的调用返回交付拒绝; 无法证明是否已接收时继续返回未知, 不仅凭队列指标宣称明确未交付.

Authority 仍保留自己的在途限制. 公告、发现和查询不会因该限制耗尽而被停止处理. 调整 Console 等待期限, 让它覆盖 Authority 内部交付等待与正常回复开销; 超时仍只能说明尚未获得结果.

已签发 Attempt 后, 登记过期、Worker 下线、拒收、无接收者或交付超时均不能自动撤销授权、强制关闭 Operation 或换人重发. 能收到的有效 Report 仍按原 Operation / Attempt 关联处理, 不因 Worker 已离开候选列表而被拒绝. 若增加报告来源检查, 对照交付时保存的目标实例, 不依赖当前登记记录是否存在.

第一版不增加跨 Authority 接管、节点表持久化、配置热重载或任务重试. 长期任务恢复继续属于独立的持久化与恢复工作.

**6. 分阶段开发与提交**

每个阶段以可编译、相应测试通过的提交为界. 协议变更跨应用协同完成, 不把中间实现描述为可混用新旧二进制的版本.

| 阶段 | 工作与涉及位置 | 完成标准 | 依赖 |
| --- | --- | --- | --- |
| P0 协议约定 | 更新 `spec/delivery.md` 的目标行为; 固定 pool 名称、身份、消息、失败回复、主题与查询约定; 增加最小共享协议定义 | node / console 共用定义可编译; 编解码、版本拒绝和主题隔离测试通过 | 本计划 |
| P1 配置与能力 | 修改 node `settings.rs`、`main.rs`、`worker.rs`; 增加能力目录和 uppercase 实现; Console 增加发现 pools 配置 | 默认配置自动生成; pools/能力去重; 未启用或未知能力不执行; uppercase 和 reverse 行为独立可测 | P0 |
| P2 Worker 登记 | 在 node `delivery` 下增加登记模块; 实现公告应用、查询过滤、过期、清理和会话隔离; 注入时间供测试 | 重复公告不增加记录; 过期立即影响候选; 旧会话下线不删除新会话; 跨池记录不互相覆盖 | P0、P1 |
| P3 公告与发现 | 接入 NATS 多 pool 普通订阅; Worker 上下线和周期快照; Authority 发现与 Worker 查询; 独立处理心跳和控制消息 | 同池多个 Authority 都收到心跳; 晚启动能重建列表; 满载仍可查询; 断开/重连及退出处理符合约定 | P2 |
| P4 定向交付 | 扩展 delivery contract、NATS 和 channel 适配器; 增加 pool/目标实例; Any 选择后走 Exact 路径; 校验 Report 来源关联 | 指定实例单选、跨池隔离、无隐式回退; 多 Authority 使用同一 Worker 时容量有界; 失败不改变既有生命周期规则 | P3 |
| P5 Console 与入口 | 将 ingress 请求/回复改为结构化数据; Authority 创建业务请求; Console 发现、过滤、编号选择和提交; 更新本地终端入口与测试适配器 | 能完成选择 Authority → pool → 能力 → Worker → 输入 → 结果; 展示实际执行身份; 选择过期时提示刷新并重新选择 | P4 |
| P6 联调与交付 | 补全场景测试、使用说明及按需运行的真实 NATS 验收脚本; 核对生命周期与登记日志 | 下述验收矩阵全部通过, 普通 Cargo 测试不依赖运行中的 NATS | P5 |

本地 Authority 终端继续作为该 Authority 的输入入口: 通过简单命令选择已加入 pool 与能力, Worker 使用同一登记选择逻辑; 初始选择保持 default/reverse 的便捷体验, 配置不包含这些值时明确提示选择. 不继续走绕过 pool 校验的旧全局主题.

Console 首版使用普通编号/命令交互即可, 不新增 TUI 框架. 多 pool 发现同一个 Authority 只展示一个实例, 选择后列出可用 pools. Worker 列表显示 label、节点/会话身份、pool、能力和状态; label 重复不影响选择正确性. 刷新后使用实例键保存选择, 不用列表序号作为网络身份.

已有 Operation / Attempt / Report 创建和发送日志保留, 补充 pool 与目标实例. 上下线、失联、恢复记录 INFO; 重复正常心跳记录 DEBUG, 避免每 5 秒刷屏. 错误保留明确未交付、未知及未解决状态的区别.

**7. 验收矩阵**

| 场景 | 预期 |
| --- | --- |
| Authority pools 为 `["a", "a", "b"]` | 只加入 a、b 各一次; 读取后配置文件字节不变 |
| Worker 只启用 uppercase | 可以执行 uppercase; reverse 不在公告及可执行列表中 |
| Authority A 加入 a、b; B 只加入 b | A 能查到 a、b 的 Worker; B 只能查到 b; b 的 Worker 可供二者使用 |
| 多个 pool 发现同一 Authority | Console 按节点/会话去重, 保留其 pool 关系 |
| Worker 晚启动、Authority 晚启动或错过上线公告 | 新完整心跳建立记录, 无需重启另一方 |
| 重复、倒序公告或旧会话 offline | 保持登记幂等, 不覆盖更新状态或新会话 |
| 不同会话使用同一 node_id | 记录与路由不串线, Console 可区分实例 |
| 心跳间隔调整, 暂停心跳超过阈值 | 按配置续期和失联, 候选过滤无需等待清理任务 |
| Worker 执行中、Authority 达到在途上限 | 心跳和列表查询仍被处理 |
| 选择后 Worker 退出、重启或开始 draining | 拒绝已知失效选择; 发生发送竞态时按证据分类失败; 不改投其他实例 |
| 两个 Authority 同时向一个 Worker 提交 | 总执行容量和缓冲仍有界, Report 回到各自原 Authority |
| 错误 pool / session / capability 的调用 | 不执行业务, 不伪造 Report |
| Worker 失联但原有效 Report 仍可到达 | 按交付时的关联接受, 不被登记过期误伤 |
| 在途交付超时、无 Report | Operation 保持 AwaitingReport, 不重试、不关闭 |
| Ctrl+C 正常退出 | draining 停止新选择, 已接收任务按原规则排空, 尽力公告 offline |
| 强制终止或 NATS 断开/重连 | 超时清除候选, 新心跳恢复视图, 无自动任务重放 |

基础验证使用 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`、`cargo test --workspace` 和 `cargo doc --workspace --no-deps`. 登记与超时测试使用可控时间, 不实际等待 5/15 秒. 复用现有 channel 和 ingress 测试验证生命周期, 增加实例与 pool 场景.

真实 NATS 验收作为显式运行的脚本, 创建临时目录和配置, 使用独立端口启动多个 Authority / Worker / Console, 结束时回收进程. Rust 构建和普通测试不依赖 Nix 或 NATS 服务端. 脚本保留在仓库, 使多 pool 订阅和定向主题行为可以重复验收.
