# Node 角色装配与配置

Node 是承载角色和基础设施的进程. 一个 Node 可以同时运行多个 Authority、多个 Worker, 也可以只连接基础设施. Authority 与 Worker 的运行状态、身份、pool、能力、心跳和容量分别配置.

## 身份与路由

| 身份 | 生命周期 | 用途 |
| --- | --- | --- |
| `node_id` | 持久保存于 Node 配置 | 定位承载进程, 输出日志 |
| Node 的 `node_session_id` | 每次进程运行重新生成 | 区分 Node 的不同运行 |
| `roles[].role_id` | 持久保存于角色配置 | Authority / Worker 的固定身份 |
| 角色的 `session_id` | 每个角色每次运行独立生成 | 锁定一次具体角色运行, 日志名为 `role_session_id` |

Console、登记表和定向交付都使用 `RoleInstance { role_id, session_id }`. `node_id` 不进入任务目标或网络路由. 同一 Node 内多个同类型角色分别发现和选择; label 可以相同, 不参与身份判断. 角色迁移到另一个 Node 时可以保留 role_id, 新运行仍生成新的会话. 同一 role_id 同时运行多个会话时分别展示, 没有隐式单活或接管.

网络协议为 v3, subject 使用 `beamlet.v3.<pool>.authority.<role_id>.<session_id>.requests` 和 `beamlet.v3.<pool>.worker.<role_id>.<session_id>.calls`. node 与 console 需一起更新. 无旧网络协议或旧配置兼容逻辑.

## 配置与首次启动

```bash
cargo build -p beamlet-node -p beamlet-console
nats-server
# 分别在终端启动:
cargo run -p beamlet-node -- --config node.ron
cargo run -p beamlet-console
```

构建与普通测试直接使用 Cargo, 不依赖 Nix. 配置文件位于**实际可执行文件旁**, 与当前工作目录无关. node 的 `--config/-c` 只接受文件名; 默认 `node.ron`. console 使用 `console.ron`.

文件不存在时 confy 创建默认配置, 同时生成一个 Node UUID 和两个角色 UUID. 默认组合为一个 Authority、一个 Worker, 共享名为 `local` 的 NATS 连接, Worker 启用 `text.reverse/v1`.

已有配置只加载和校验, 不更新、不补写 UUID、不迁移. 后续添加角色时由用户填写新的 role_id; 删除或调整其他角色顺序不影响剩余角色身份. 缺失、nil 或同一个配置中重复的 role_id 都会在连接网络前报错. 所有会话 UUID 只存在内存中.

以下仅展示配置结构. 添加角色时为每个角色填写自己的 UUID; `uuidgen` 可用于生成. 角色种类采用 RON 带数据的枚举: `authority((...))`、`worker((...))`.

```ron
(
    node_id: "76da5097-794c-4306-bd45-caa6a501c200",
    label: Some("开发节点"),
    color: auto,
    shutdown_timeout_secs: 6,
    infrastructures: {
        "local": nats(url: "nats://127.0.0.1:4222", connect_timeout_secs: 10, subscription_capacity: 16),
        "secondary": nats(url: "nats://127.0.0.1:4333", connect_timeout_secs: 10, subscription_capacity: 16),
    },
    roles: [
        (
            role_id: "3e8385b1-a50b-4cbd-a60f-8469e7e6ff3a",
            label: Some("协调者 A"),
            infrastructure: "local",
            kind: authority((
                pools: ["blue", "green"],
                worker_timeout_secs: 15,
                report_timeout_secs: 3,
                input_capacity: 16,
                max_concurrent_replies: 16,
                max_concurrent_tasks: 16,
            )),
        ),
        (
            role_id: "99ab9db5-4b2e-4285-bad2-546a64d0baac",
            label: Some("协调者 B"),
            infrastructure: "local",
            kind: authority((
                pools: ["blue"],
                worker_timeout_secs: 20,
                report_timeout_secs: 3,
                input_capacity: 16,
                max_concurrent_replies: 16,
                max_concurrent_tasks: 0,
            )),
        ),
        (
            role_id: "71d3e189-0478-4e8b-bfd2-93cdfd975dd7",
            label: Some("大写 Worker"),
            infrastructure: "local",
            kind: worker((
                pool: "blue",
                capabilities: ["text.uppercase/v1"],
                heartbeat_interval_secs: 5,
                max_concurrent_tasks: 4,
            )),
        ),
        (
            role_id: "5c8fac39-1d02-49f9-8b2b-a21b5793a867",
            label: Some("倒写 Worker"),
            infrastructure: "secondary",
            kind: worker((
                pool: "blue",
                capabilities: ["text.reverse/v1"],
                heartbeat_interval_secs: 3,
                max_concurrent_tasks: 0,
            )),
        ),
    ],
)
```

`infrastructures` 以名称标识实例, 当前实现 NATS 客户端连接. 相同类型可以配置多次, 指向同一服务器或不同服务器. 多个角色引用同一个名称时共享连接; 不同名称建立独立连接. 它不会自动启动 NATS 服务端. 此例中第二个 Worker 位于另一台 NATS, 要使用它, 需要在 `secondary` 上配置 Authority. 同名 pool 不能跨独立 NATS 自动互通.

每个角色引用一个基础设施, 不做跨连接转发. 不存在的名称、空地址、空 Authority pools、未知 Worker 能力、零心跳间隔或零期限均为配置错误. pool 和能力去重只作用于内存. 日志颜色属于整个 Node 的终端输出设置, 使用 `auto/always/never`.

## 本地终端与 Console

一个 Node 只创建一个 stdin 读取入口. 只有一个 Authority 时自动选择它; 多个 Authority 时先明确选择:

```text
/roles
/authority 2
/pool blue
/capability text.uppercase/v1
Hello Beamlet
```

`/authority` 也接受完整 role_id. 之后的文本和 `/pool`、`/capability`、`/workers` 命令只交给选中的角色. 切换回来时各 Authority 保留自身的 pool / 能力选择. 本地 EOF 不结束网络服务. 接收队列满时该次本地输入明确拒绝, 不转交别的角色.

Console 继续使用 `/refresh`、`/authority <编号>`、`/pool <编号或名称>`、`/capability <编号或标识>`、`/worker <编号或 any>` 选择执行目标. 列表和执行结果展示 role_id 与角色会话; 同一个 Node 内的 Authority 不合并. Console 配置的 NATS 地址与 pools 决定发现范围.

Console 将输入、发现、查询和提交结果作为独立事件处理, 等待执行结果时仍可提交新任务、查询或切换目标. 每次提交固定当时的 Authority、pool、能力和 Worker 选择, 结果按完成顺序展示并带 request_id. 旧提交的拒绝不会撤销后来的选择; 查询乱序完成时只采用当前目标的最新查询结果. Console 的 `max_in_flight_requests` 限制在途请求数, 默认 16 (包括发现和查询), 满载时明确拒绝新的网络请求, 本地选择、帮助和退出仍可使用. 此上限独立于 Authority / Worker 配置.

`/quit` 和 EOF 停止新输入并等待在途请求完成或达到已有期限. Ctrl+C 停止本地等待, 已发送请求仍可能在远端执行; 不会自动重发或宣称撤销 Attempt.

## Authority 并发

每个 Authority 角色也通过 `max_concurrent_tasks` 独立配置, 默认 `16`, 正数限制同时交付并等待报告的任务数, `0` 不设该上限. 例如:

```ron
kind: authority((
    pools: ["default"],
    worker_timeout_secs: 15,
    report_timeout_secs: 3,
    input_capacity: 16,
    max_concurrent_replies: 16,
    max_concurrent_tasks: 32,
)),
```

该上限跨当前 Authority 的所有 pools 和 Worker 共享, 其他 Authority 有自己的名额. 满载时暂停从业务入口取出请求, 发现、心跳与 Worker 查询继续由独立控制循环处理. 报告可以乱序完成; 交付完成或失败后释放名额. 超时释放并发名额不表示取消 Attempt 或关闭 Operation: 原操作仍保持等待报告, 不自动重试.

Operation 创建、报告接受和状态变更仍由 Authority 主循环逐个执行, 异步交付并发等待. 回复通过独立异步任务发送, 单个慢回复不占住主循环. 每个 Authority 通过 `max_concurrent_replies` 限制待发送的业务结果/拒绝回复, 默认 16; 达到该上限时暂停业务接收和报告收取, 等待发送释放容量, 不无限积压回复. 停止时不再签发新任务, 丢弃未签发的缓冲请求, 等待已经发出的交付和回复发送结束.

每个 Authority 的本地和远端业务入口分别按 `input_capacity` 缓冲, 默认各 16 条, NATS 每个订阅的缓冲由 `subscription_capacity` 指定, 默认 16, 与执行并发设置无关; `0` 不把接收队列改为无界, 也不会取消 Worker 自己的执行限制. 已有配置需手动为每个 Authority 添加 `max_concurrent_tasks`, 重启生效, 加载器不补写文件.

## Worker 并发

每个 Worker 角色通过 `max_concurrent_tasks` 独立配置:

| 值 | 行为 |
| --- | --- |
| `1` | 串行执行, 首次生成配置的默认值 |
| 大于 `1` | 最多同时处理指定数量的任务 |
| `0` | Worker 不设置并发上限 |

名额从取出调用开始占用, 到执行结束且报告发送完成或失败后释放. 达到正数上限时暂停取出下一条调用, 不额外创建大量等待名额的执行任务. 所有 Authority 发给该 Worker 的调用共用此上限; 其他 Worker 角色有自己的名额.

能力执行接口返回 Future, I/O 能力可以直接异步等待; 现有文本能力自行将计算放入阻塞线程池. Worker 统一管理执行和报告发送, 挂起的任务仍占用并发名额, 心跳与控制循环继续运行. `0` 不等于无限线程或硬件资源: Tokio 的线程池、系统资源、每个 Authority 配置的在途上限和传输接收缓冲仍独立生效. Worker 接收缓冲由其 NATS 实例的 `subscription_capacity` 决定, 默认 16 条, 与执行并发数是不同设置.

Report 身份按接收顺序分配, 报告按完成顺序返回, 不等待更早的慢任务. 执行拒绝、报告发送失败都会释放名额, 不触发重新执行. 收到退出信号后停止新接收, 按当前并发上限排空已接收队列并等待运行中任务; 仍受 Node 的统一退出期限约束.

已有配置需要手动在各 Worker 的 `kind: worker((...))` 中添加 `max_concurrent_tasks`, 修改后重启生效. 加载器不会补字段或改写文件.

## 运行管理和退出

Node 先校验整个配置、生成各角色会话并连接基础设施, 再把已装配的角色 future 交给 [beamlet-host](../crates/beamlet-host/src/lib.rs) 统一启动和监督. 每个 Authority 独立管理 Operation、登记表和在途上限; 每个 Worker 有自己的并发上限和有界接收队列. 基础设施共享不共享角色业务状态.

连接中断由对应 NATS 实例重连, 不终止其他基础设施的角色. 某个角色不可恢复地退出或启动失败时, Node 通知所有角色停止新接收并排空, 然后返回失败; 不在进程内偷偷重启角色. Ctrl+C 同样广播一次退出请求, 每个 Worker 独立公告 draining / offline. 按 `shutdown_timeout_secs` 等待角色排空, 默认 6 秒, 超时返回失败并停止等待; 已开始的阻塞执行不能强行取消, 任务也不会自动重试. 宿主汇总首次失败、排空期间的其他错误和超时未完成任务, 以角色 UUID 定位; 后续错误不重置排空期限. 库发送 `host.stopping` 日志, Node 保留 `node.stopped`. 退出时对每个连接最多等待 1 秒刷新, 刷新不保证远端已收到.

配置修改需要重启. 当前没有角色热增删、角色自动重启、动态插件或任意基础设施类型注册框架. 新角色类型通过带配置的枚举分支和角色运行入口扩展; 新基础设施实现通过对应配置和连接入口扩展.

## 验证

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
# Unix, 需要 PATH 中的 nats-server; 普通测试默认忽略:
cargo test -p beamlet-node --test composition -- --ignored --nocapture
cargo test -p beamlet-console --test concurrency -- --ignored --nocapture
```

组合验收同时启动两台 NATS, 在一个 Node 中运行 3 个 Authority、3 个 Worker 和 3 个基础设施连接, 检查同类型多实例、角色定向、能力隔离、连接隔离、本地选择、重连、角色跨 Node 运行、旧会话退出不影响新会话、所有角色排空和配置不变. 身份测试通过真实二进制验证重启前后各 UUID 的持久和临时边界. Worker 并发测试使用可控异步挂起验证串行、有限并发、无限模式、乱序报告、名额释放与退出排空. Authority 入口测试覆盖可配置有限上限、超过默认 16 的无限模式、角色名额隔离和失败后释放名额. 普通测试不需要 NATS 服务端.

Console 并发测试验证等待中的提交不阻止查询、目标快照、迟到结果隔离、容量拒绝、退出排空与中止等待. 真实 NATS 验收使用 Console 二进制和可控 Authority 回复, 验证多个提交和查询同时在途、乱序结果关联和配置不变. Authority 回复测试通过有界通道制造发送背压, 验证其他任务可继续、回复容量有限和停机排空.

## 可配置期限和容量

Node 顶层 `shutdown_timeout_secs` 默认 6; 每个 Authority 的 `report_timeout_secs` 默认 3. NATS 实例的 `connect_timeout_secs` 默认 10, 同时约束首次连接整体等待与客户端单次连接; `subscription_capacity` 默认 16, 分别作用于该连接上的每个普通订阅, 不表示连接共享的总消息数.

Console 顶层 `request_timeout_secs` 默认 10, `connect_timeout_secs` 默认 10, `subscription_capacity` 默认 65536 (延续原客户端默认值). 请求期限同时传给客户端与外层等待, 发现也受整体请求期限约束. 超时与容量均须大于 0 且在系统支持范围内. 调大 Report 期限后应给 Console 留出排队和回复余量; Node 退出期限独立决定愿意等待角色多久. 超时不取消远端执行, 不自动重试.

这些字段在首次生成配置时写入. 已有配置必须手动补全, 缺失或无效值会在启动前报错; 不迁移或改写已有配置.

Authority 的 `input_capacity` 默认 16, 分别用于该角色的本地输入和远端业务队列; `max_concurrent_replies` 默认 16, 限制主循环已处理完毕但尚未发送完成的回复. 控制循环中的发现、查询和队列满拒绝仍直接发送, 不计入此回复上限. 两者均要求大于 0.

Console 的 `max_in_flight_requests` 默认 16, 要求大于 0; `discovery_window_ms` 默认 1000, 从发送完发现查询后开始收集回复, 必须大于 0 且小于 `request_timeout_secs` 的总期限, 给连接发送等步骤留出余量. 完整 Console 配置示例:

```ron
(
    nats_url: "nats://127.0.0.1:4222",
    color: auto,
    pools: ["default"],
    request_timeout_secs: 10,
    discovery_window_ms: 1000,
    max_in_flight_requests: 16,
    connect_timeout_secs: 10,
    subscription_capacity: 65536,
)
```

仅 Authority / Worker 的 `max_concurrent_tasks` 接受 `0 = 无限`. 输入队列、订阅缓冲、回复发送并发、Console 在途上限和超时均不采用无限语义. Node / Console 终端读取线程的缓冲仍固定 16 行; 心跳发送及退出刷新期限、登记清理频率和保留倍数、协议名称长度规则继续由内部控制.

配置期限的真实 NATS 测试覆盖不同 Authority 的独立 Report 期限、超过原 3 秒仍接受报告、短期限返回未解决且不重试、业务队列满拒绝和 Console 的非默认请求超时. 容量测试分别使用 1、3、32 验证本地输入队列、Authority 回复发送与 Console 在途请求的限流和恢复.

## 嵌入宿主

其他程序可以使用 `Host::new(options)`、`host.shutdown()` 和 `host.add(name, future)` 管理任意长期任务, 无需 node 的角色配置枚举. `add` 不启动任务, `run(signal)` 开始轮询才启动. `run_with_background(signal, input)` 允许输入处理正常结束后服务继续运行, 输入失败则停止组; 排空开始时释放输入 future. 任务在停止请求前正常结束也视为意外退出.

宿主不连接消息服务、不创建 Tokio 执行器、不监听 Ctrl+C, 由应用完成这些工作. 丢弃运行 future 会广播停止并请求中止异步子任务, 不等待排空; 正常排空需完成 signal 后等待 run 返回. 任务内部启动的阻塞计算和远端调用不因此被强制取消. 运行 `cargo run -p beamlet-host --example composed` 可验证共享/隔离消息空间中的多角色嵌入, 无需配置文件或服务器.
