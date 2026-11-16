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
    infrastructures: {
        "local": nats(url: "nats://127.0.0.1:4222"),
        "secondary": nats(url: "nats://127.0.0.1:4333"),
    },
    roles: [
        (
            role_id: "3e8385b1-a50b-4cbd-a60f-8469e7e6ff3a",
            label: Some("协调者 A"),
            infrastructure: "local",
            kind: authority((
                pools: ["blue", "green"],
                worker_timeout_secs: 15,
            )),
        ),
        (
            role_id: "99ab9db5-4b2e-4285-bad2-546a64d0baac",
            label: Some("协调者 B"),
            infrastructure: "local",
            kind: authority((
                pools: ["blue"],
                worker_timeout_secs: 20,
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

## 运行管理和退出

Node 先校验整个配置、生成各角色会话并连接基础设施, 再启动角色. 每个 Authority 独立管理 Operation、登记表和在途上限; 每个 Worker 有自己的串行执行和有界队列. 基础设施共享不共享角色业务状态.

连接中断由对应 NATS 实例重连, 不终止其他基础设施的角色. 某个角色不可恢复地退出或启动失败时, Node 通知所有角色停止新接收并排空, 然后返回失败; 不在进程内偷偷重启角色. Ctrl+C 同样广播一次退出请求, 每个 Worker 独立公告 draining / offline. 默认最多等待角色排空 6 秒, 超时返回失败并停止等待; 已开始的阻塞执行不能强行取消, 任务也不会自动重试. 退出时对每个连接最多等待 1 秒刷新, 刷新不保证远端已收到.

配置修改需要重启. 当前没有角色热增删、角色自动重启、动态插件或任意基础设施类型注册框架. 新角色类型通过带配置的枚举分支和角色运行入口扩展; 新基础设施实现通过对应配置和连接入口扩展.

## 验证

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
# Unix, 需要 PATH 中的 nats-server; 普通测试默认忽略:
cargo test -p beamlet-node --test composition -- --ignored --nocapture
```

组合验收同时启动两台 NATS, 在一个 Node 中运行 3 个 Authority、3 个 Worker 和 3 个基础设施连接, 检查同类型多实例、角色定向、能力隔离、连接隔离、本地选择、重连、角色跨 Node 运行、旧会话退出不影响新会话、所有角色排空和配置不变. 身份测试通过真实二进制验证重启前后各 UUID 的持久和临时边界. 普通测试不需要 NATS 服务端.
