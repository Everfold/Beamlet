# 实现任务与提交

状态: 2026-11-10, 最近可执行任务. 总体顺序见 [实现计划](implementation-plan.md), 语义以 [设计入口](README.md)及关联契约为准. 每实现一个任务并通过验证, 单独 commit.

## 提交规则

一个任务解决一个可观察的问题, 包含实现, 调用点迁移, 必要测试与相关文档. 每个提交保持可编译, 现有行为在明确变更范围外保持不变; 不先提交断开的接口或失败测试, 再依赖下个提交修复. 单个任务若同时改变多套独立规则, 在动手前继续拆分.

验证通过后只暂存本任务文件并提交, 记录 commit 与验证结果, 然后进入下一项. 已有不相关改动不夹带, 不默认 push. 测试迁移到拥有行为的库, 保留应用装配验收; 不复制整套测试作为分层证明.

项目处于新建阶段, API, CLI 和原型协议直接随设计更新; 不保留旧名称重导出, 参数别名或旧编码映射.

角色与基础设施通过公开接口由应用显式组装. 可复用实现归库, Node/Console 保留参数, 交互, 装配, 信号接入与展示. 独立角色无需统一 Node 容器, 本地内存执行无需 Zenoh 或数据库.

## 近期任务

### T1. 将独立 Authority/Worker 驱动迁入 beamlet

状态: 已实现; 独立驱动与六项消息流程测试已迁入 beamlet, 调用方直接依赖 beamlet. 全工作区测试, Clippy 与相关库文档检查通过.

- 范围: 将 `agent/beamlet-agent/src/authority.rs` 与 `worker.rs` 的既有实现迁入 `kernel/beamlet`, 通过 `beamlet::Authority` 与 `beamlet::Worker` 提供. 原 Agent crate 不保留转导出. 迁移独立角色消息流程测试.
- 验收: 仅依赖 beamlet 的调用者可以显式构造两种角色, 交付授权并收录结果; 重复投递, 等待取消, 错误绑定和乱序返回维持既有语义.
- 提交: `beamlet: move authority and worker drivers into library`.

### T2. 整理本地组合入口

状态: 已实现. LocalExecution 提供 from_parts/into_parts 显式组装与拆分, 调用方直接使用通用执行库; 演示与组合行为测试已迁移. 全工作区测试与仓库 Clippy 检查通过, 覆盖组装前后状态保留及错误绑定拒绝.

- 范围: 将旧 `Agent` 的本地执行组合迁入 beamlet, 使用不会与领域 Agent 混淆的名称. 提供显式组装入口, 直接更新已有调用点.
- 验收: 独立角色与可选组合入口使用同一实现; 接纳后派发失败仍保留原接纳事实; 取消等待不会重跑或取消已开始执行; 新入口不隐式建立基础设施.
- 提交: `beamlet: expose explicit local execution composition`.

### T2a. 移除原型兼容入口

状态: 已实现; 移除 Agent/AgentError 及通用类型转导出, Node 的测试模块包装与 --role local 别名; 应用和测试直接依赖所属库.

- 验收: 工作区调用方使用当前接口; 组合行为测试保留在 beamlet, 不再维护旧入口测试.
- 提交: `refactor: remove prototype compatibility entry points`.

### T3. 统一影响字段的领域命名

状态: 已实现; 领域与原型 Adapter 使用 ImpactStatus/impact, 错误名称与 Console 显示同步更新. JSON 仅接受 impact 字段, 无旧字段别名或转换层. 工作区检查, 证据规则与编码测试, Node/Console 经 Zenoh 的实际往返验证通过.

- 范围: 按已确认设计将领域类型 `Effect` 调整为 `ImpactStatus`, 字段使用 `impact`, 更新库调用点. 原型 JSON 同步改为 impact, 不提供 Effect/effect 别名或旧字段映射.
- 验收: 无影响/已观察/未知与成功/失败独立; 当前消息采用 impact 字段往返; 拒绝与证据校验语义不变.
- 提交: `kernel: name external impact evidence explicitly`.

### T4. 下沉按提交键接纳的通用能力

状态: 已实现; SubmissionKey 与接纳映射由通用 Authority 账本持有, Node 已调用 submit_keyed/check_submission, 不再维护提交去重表. 六项库用例覆盖去重, 定义冲突, Authority 隔离, 终态保留, 容量拒绝与 ID 碰撞后的原子性. 工作区测试, 全功能 Clippy, 文档与示例检查通过; 三个独立 Console 连接验证同键复用全部执行 ID, 换键创建新执行, Worker 共执行两次并正常排空退出.

- 范围: 从已有固定任务提交路径提炼 SubmissionKey 到 Operation/Attempt 的关联规则, 放入通用执行库; Authority 仅在新接纳时调用应用显式提供的身份生成函数. 协议 Adapter 将库返回的原授权投影为接纳回复.
- 验收: 同 Authority 同键同内容返回原执行, 同键异内容拒绝, 相同内容不同键产生不同执行; 接纳失败不留下半份键映射; 核心逻辑无需 Zenoh 或应用 crate.
- 提交: `beamlet: centralize keyed operation admission`.

### T5. 提供本地 Client 提交与查询

状态: 已实现; LocalClient 借用应用显式提供的 Authority, Worker 和 ID 生成函数, LocalExecution 提供相同绑定的便捷入口. 提交与三类查询返回独立快照, 交付失败明确携带已接纳执行的绑定; 共享演示已迁移至 Client. 五项库用例覆盖回执找回, 去重与冲突, 未知查询和 Authority 隔离, 入队失败不重投, 错误角色绑定及迟到/附加证据. 全工作区测试, 全功能 Clippy 与库文档检查通过. 当前仅提供同进程内存绑定, 远端查询/订阅及持久恢复仍待后续任务.

- 范围: 通过显式本地绑定实现提交, 原提交键查询, Operation 查询及 Report 查询. 确定最小结构化返回与错误, 不在同一提交加入完整远端订阅或持久恢复.
- 验收: 库调用者通过 Client 完成提交和结果查询; 回执丢失可按原键找回执行; 未知引用与冲突明确区分; 查询不创建执行; 应用不组装底层授权和信封.
- 提交: `beamlet: add local execution client queries`.

### T6. 将远端 Client 提交流程下沉到库

状态: 已实现; remote::Client 拥有原键重投, 回执关联及结果等待, Console 已迁移为显式装配和结构化结果展示. 六项通用驱动测试及两项 Zenoh/编码测试通过, 覆盖同键连续调用时的迟到回执, 固定预算, 取消等待和共享会话保留. 全工作区测试, 全功能 Clippy 与库文档检查通过; 三个实际 Console 会话验证同键复用全部执行 ID, 换键独立执行, 生命周期订阅共存, Worker 共执行两次且 Node 正常排空退出. 远端纯查询和观察合并分别留给 T7/T8.

- 范围: beamlet 提供独立于编码与连接实现的有界提交驱动; Zenoh Adapter 绑定应用已有连接和目标; Hello 编码 Adapter 转换类型. Console 负责参数, 装配, 交互与展示.
- 验收: 库调用者无需启动 Console CLI 即可完成远端提交; 编码一次并沿用原键与既定预算; Report 先到能确认原映射; 错绑定和非法消息不确认; 接纳后停止重投; 超时或断线保留原键与已知映射; Client 释放后共享连接仍可使用.
- 提交: `beamlet: extract remote submission client from console`.

### T7. 提供远端只读查询入口

状态: 已实现; Authority::answer_query 以只读借用提供三类查询, remote::Client 使用独立查询关联与明确 Authority, 返回与本地相同的执行快照或完整证据. Zenoh 绑定和 Hello 编码复用为 ClientTransport/HelloClientCodec, 无旧名称兼容入口; Node 仅接入查询路由, Console 提供三种查询命令. 六项通用查询测试及两项 Zenoh/编码测试通过, 覆盖重连找回, 迟到关联, 非法回复, 原等待状态与附加证据, 网络失败及未知记录不触发提交. 工作区 121 项测试(含文档测试), 全功能 Clippy 与库文档检查通过. 实际 Node/Console 验证新进程可找回原键/Operation/Report, 三类未知记录与不可达 Authority 明确区分, 未知键查询后仍可提交; 两次显式提交仅执行两次, Node 正常排空退出.

- 范围: 将原键, Operation 和 Report 查询接入 Zenoh, Authority 查询处理归库, Console 调用并展示. 如编码和服务接入超过单项规模, 实施前继续拆分.
- 验收: 重新连接后按原键找回执行; 查询不创建或重新交付执行; 未找到与网络不可达明确区分; 返回事实与本地查询一致.
- 提交: `beamlet: add remote read-only execution queries`.

### T8. 合并结果观察并通过查询补齐

状态: 已实现; observe::State 单调合并执行快照和已收录通知, 按 ReportId 去重并保留 Authority 采用决定, 同一内存 State 至多交付一次采用报告. remote::Client::observe 接受显式通知通道及预算, 开始时和等待期间通过只读查询补齐; 取消停止 I/O, 断线后保留 State 可重新绑定, 不重新提交执行. Node 收录后发送有界尽力通知, Console 新增 --observe-operation 并直接调用库. 十项核心观察测试及两项新增 Zenoh/编码测试覆盖所有输入顺序, 冲突/容量原子性, 附加报告, 通知遗漏, 重连补齐, 连续无效消息下的总期限和取消等待. 工作区 133 项测试(含文档测试), 全功能 Clippy 与库文档检查通过; 实际 Console 观察返回原报告一次, 原提交及查询仍正常, 两次显式提交仅执行两次且 Node 正常排空退出. 当前每次驱动跟踪一个 Operation, 消费状态仍在内存中; 自动重建连接, 持久消费与共享队列多执行分流另行实施.

- 范围: 库合并结果查询与通知, 处理重复及倒序消息, 以有界查询补齐遗漏的结果. 应用消费结构化观察.
- 验收: 订阅前完成仍能查到; 旧消息不回退已知事实; 相同 Report 不重复消费; 通知遗漏或重新连接后查询补齐, 不以新提交替代查询.
- 提交: `beamlet: reconcile result notifications with bounded queries`.

### T9. 下沉 Attempt 的交付与重投驱动

状态: 已完成. 通用 `attempt_delivery::Outbox` 已承接原授权交付, Node 仅保留显式组装及 I/O. 新增 7 项库行为测试和 1 项真实 Zenoh 测试; 工作区 141 项测试, 全功能 Clippy 和文档检查通过. 独立进程验证 Worker 延迟上线后重投原授权且只执行一次, 缺席时三次耗尽且重复提交不刷新预算; 本地提交, 查询, 观察与排空回归通过.

- 范围: 将 Node 的原授权交付表移入 beamlet, 由库持有完整定义校验, 编码一次, 固定发送预算, 确认与耗尽观察; 应用提供编解码, 路由, 连接及发送任务集合. 删除 Node 原模块, 不保留兼容入口.
- 验收: 回执丢失只重投原身份和字节, 不重复执行; 错绑定/定义冲突不改变已有记录; Authority 成功收录 Report 才能停止对应重投; 迟到回执仍可确认, 耗尽后重复加入不恢复预算; 旧发送完成不能释放后续发送占用. 本地交付保持类型化直接返回.
- 提交: `beamlet: extract bounded attempt delivery from node`.

## 后续拆分时点

T9 完成后继续细拆原 Report 交付驱动, 生命周期观察与 Node 显式组装. 提交重投, Attempt 重投, Report 重投和生命周期迁移分别验收, 不合成一次全量搬迁.

执行底座可独立使用后, 依次细拆存储事务与恢复, Agent 数据与 Loop, 回答查询与通知, 输入投影与 fork, 工具/Codemode 集成. 每项都沿用同一提交规则. 不提前实现 Sub-Agent, MCP Sampling 或跨节点自动接管.

进入存储阶段前对齐后端选择: 当前 workspace 已声明 `kite_sql`, `beamlet-durable` 仍为空依赖脚手架, 设计文档仍写 Diesel + SQLite. 新依赖声明尚不等于持久化契约已经实现, 本批执行入口任务不修改后端选择.

## 验证约定

Rust 改动运行格式检查, 相关库与调用方测试, Clippy 和库文档检查, 并完成仓库要求的检查. 验证范围随实际影响确定; 修改代码或出现失败后重跑相应检查. 提交说明记录结果与未实现范围, 不将文件迁移描述为新增可靠交付或持久恢复能力.
