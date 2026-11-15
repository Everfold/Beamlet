# 能力交付契约

状态: 首版已实现并验收. node 可独立启动 Authority / Worker, 通过 Core NATS 按能力单选交付; channel 实现用于共用契约测试, 不作为运行时回退. 本文约束行为, 不限定实现语言; 启动命令见[仓库说明](../README.md).

## 1. 职责与范围

领域原语仍只有 [Operation, Attempt, Report](kernel.md). 能力与角色是外层描述, 不新增生命周期原语.

Authority 管理 Operation, 签发 Attempt, 接受 Report 并决定后续动作. Worker 声明能力, 执行请求并生成 Report. 交付模块负责把 Attempt 与原始请求所需数据交给一个匹配的 Worker, 将完整 Report 路由回原 Authority.

交付模块先放在 node 内部, 不新建 crate. 第二个应用需要复用, 或 channel 与 NATS 实现验证出稳定接口后再考虑提取. kernel / runtime 不依赖交付模块; 模块通过已有原语与它们协作.

### 请求入口与回复

node 内部的 ingress 模块负责外部请求接收和最终文本回复, 与面向 Worker 的 delivery 分开. Authority 只取得有效文本及一次性、不透明的回复入口, 不接触终端、NATS 连接、主题、编码或回复地址. ingress 不创建 Operation / Attempt, 最终文本回复也不是 Worker 的 Report.

线上入口合并本地终端与 Core NATS: 外部主题仍为 `beamlet.reverse`, Authority 队列组仍为 `beamlet-nodes`. 空行与空白原样保留; 非 UTF-8 请求由入口回复输入错误, 无回复地址的消息被忽略. 本地 EOF 只结束本地输入, 读取错误仍向上传播.

接收等待被取消后可继续, 不能因此丢失有效请求或重建已开始的拒绝回复. 入口不额外建立后台请求队列; Authority 仍最多同时等待 16 次交付. 停止接收后不再交出缓冲请求, 但已交出的回复入口仍有效. Authority 等待已签发交付结束后刷新回复缓冲; 刷新不证明原调用者已收到. 全部输入结束也走这个退出流程, 但线上 NATS 订阅意外关闭仍是错误.

有界内存入口只用于测试真实 Authority 循环, 不替代线上 NATS, 不增加运行时回退. CLI、消息格式、交付失败不重试及等待报告状态的语义保持不变.

## 2. 能力匹配与单个接收者

第一版每个请求需要一个带版本的能力标识, 例如 `text.reverse/v1`; Worker 可以声明多个能力. 标识精确匹配, 版本约定输入、输出与业务行为, 不使用隐式兼容或通配匹配.

- A、B 支持倒写, C 只支持图片缩放: 倒写调用只交给 A 或 B 中的一个.
- 一次交付只选择一个接收者, 不广播任务让 Worker 自行过滤. 不承诺轮询、公平性或完成顺序.
- 交付模块不自动重发或换人. Authority 也不得因等待超时重复交付同一 Attempt.
- 未来要求多个能力时, 接收者必须全部满足; 不能向各能力分别发送一份调用. 第一版不提供组合条件接口.
- Worker 声明必须与真实实现一致, 执行前仍校验能力版本及请求格式. 声明、Worker 身份与 Report 关联检查都不是身份认证.

这里只约束一次交付的接收者选择. 不保证断线、重启或重复调用下业务恰好执行一次; 会重投的传输实现必须额外满足此契约, 不能直接替换后宣称语义不变.

## 3. 交付接口与失败语义

接口表达三件事: Worker 提供能力并接收调用; Authority 按能力交付 Attempt 与请求; Worker 返回完整 Report. 暂不固定函数签名. subject、queue group、连接、编码、回复地址和 Worker 到达信息均由适配器管理, 不进入领域原语或 Authority 的交付调用参数.

| 观察结果 | 含义 |
| --- | --- |
| 已提交 | 传输接受了本次提交, 不证明 Worker 已收到或执行 |
| 当前无接收者 | 有明确证据表明此次没有可接收调用的匹配 Worker, 不代表永久不存在 |
| 明确未交付 | 能证明没有交给任何 Worker, 例如发送前编码失败 |
| 交付结果未知 | 可能已收到或执行, 例如提交后的断线或等待超时 |
| 收到 Report | 收到执行结束的结果证据, 仍需 Authority 校验并接受 |

适配器只报告有证据支持的结果, 无法确认时返回未知. Worker 的业务失败应由真实 Report 表达, 不与传输失败混为一谈.

任何交付结果都不直接改变 Operation 状态. 已签发 Attempt 后即使明确未交付, 当前内核也没有撤销授权的转换: Operation 仍为 AwaitingReport. 不伪造 Report、不强制关闭、不签发替代 Attempt. 能力预检不能消除签发与发送之间的失败窗口; 无报告时如何收尾另行设计.

超时不取消 Worker 执行. 迟到 Report 若仍可收到, 按原 Operation / Attempt 身份处理; 不承诺跨断线或退出找回报告. 分布式身份由外层保证其作用域内唯一, 包括多 Authority、多个 Worker 及重启后的隔离, 不限定 UUID.

## 4. Worker 登记与适配器

需要的是匹配 Worker 的信息, 不是一张持有所有节点状态的全局表. node 是部署程序, Authority / Worker 是职责; 交付模块只关心 Worker 是否可接收符合能力的调用.

| 实现 | 能力与接收者信息 | 单个接收者的实现 |
| --- | --- | --- |
| channel | 交付模块内部维护本地 Worker 能力及发送端 | 匹配后只向选中 Worker 的有界 channel 发送 |
| Core NATS | 适配器将能力标识映射到唯一主题, 支持该能力的 Worker 加入同一队列组 | 每次只向该主题发送一份, 由 NATS 在组内选择接收者 |

NATS 映射必须避免能力名称碰撞. 受控任务主题下不能出现额外普通订阅或不同队列组, 否则会产生额外接收者. 此约束属于适配器部署契约, 不是 kernel 的保证. 两种实现都需限定在途调用, 不能用无界排队掩盖满载.

第一版不另建中心登记表、心跳系统或调度服务. channel 的本地登记不是全局节点表; NATS 的订阅信息也不是完整、可查询的 Worker 名册. channel 测试适配器已实现实例内的能力登记与单选, 不作为生产节点名册.

需要按负载、地区、组合能力选人或展示 Worker 清单时, 再引入交付模块内部的 Worker 登记表. 最小信息为 Worker 实例身份、能力、到达信息和可用性期限. 登记过期只影响后续选择, 不证明已有任务停止, 不允许把在途 Attempt 自动转交他人. Worker 登记表不保存或代管 Operation 生命周期.

## 5. 验收

请求入口重构另由[内存入口与 Authority 测试](../apps/beamlet-node/src/ingress/tests.rs)及[真实 NATS 入口测试](../apps/beamlet-node/src/ingress/nats_tests.rs)覆盖: 并发乱序回复、交付错误及无效报告、满载后释放容量、停止接收与在途等待、回复失败、终端 EOF、输入校验及队列组单选. `cargo test -p beamlet-node` 运行内存测试; 安装 `nats-server` 后运行 `cargo test -p beamlet-node -- --include-ignored` 可一并执行全部真实 NATS 测试.

仓库保留了[共用契约测试](../apps/beamlet-node/src/delivery/tests.rs)与[Authority 生命周期测试](../apps/beamlet-node/src/authority.rs). 共用场景在 channel 和隔离 NATS Server 上运行; 队列满载的确定性检查由 channel 完成. 本轮还通过真实进程验收检查了多 Authority / Worker、超大负载的发送前拒绝及接收后失联, 但该验收脚本未保留在仓库中. 这些检查不是任意故障下的恰好一次证明.

1. A、B 能倒写, C 只能缩放: 一次倒写调用恰有一个匹配 Worker 收到, C 收不到.
2. 能力版本不匹配或没有接收者: 不执行不支持的工作; 只在有证据时报告无接收者.
3. Worker 在提交前后退出、排队满载或等待超时: 正确区分明确未交付与未知, 不自动重发, Operation 不被错误结束.
4. 多 Authority 并发及 Report 乱序、迟到、重复: 回复不串线; 关联检查与生命周期规则不变.
5. 更换 channel / NATS 实现: Authority 的交付调用不出现消息服务专有概念, 不改 kernel / runtime.
