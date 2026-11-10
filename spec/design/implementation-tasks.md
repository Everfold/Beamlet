# 实现任务与提交

状态: 2026-11-10, 最近可执行任务. 总体顺序见 [实现计划](implementation-plan.md), 语义以 [设计入口](README.md)及关联契约为准. 每实现一个任务并通过验证, 单独 commit.

## 提交规则

一个任务解决一个可观察的问题, 包含实现, 调用点迁移, 必要测试与相关文档. 每个提交保持可编译, 现有行为在明确变更范围外保持不变; 不先提交断开的接口或失败测试, 再依赖下个提交修复. 单个任务若同时改变多套独立规则, 在动手前继续拆分.

验证通过后只暂存本任务文件并提交, 记录 commit 与验证结果, 然后进入下一项. 已有不相关改动不夹带, 不默认 push. 测试迁移到拥有行为的库, 保留应用装配验收; 不复制整套测试作为分层证明.

角色与基础设施通过公开接口由应用显式组装. 可复用实现归库, Node/Console 保留参数, 交互, 装配, 信号接入与展示. 独立角色无需统一 Node 容器, 本地内存执行无需 Zenoh 或数据库.

## 最近五个任务

### T1. 将独立 Authority/Worker 驱动迁入 beamlet

状态: 已实现; 独立驱动与六项消息流程测试已迁入 beamlet, Agent crate 通过重导出兼容旧入口. 全工作区测试, Clippy 与相关库文档检查通过.

- 范围: 将 `agent/beamlet-agent/src/authority.rs` 与 `worker.rs` 的既有实现迁入 `kernel/beamlet`, 通过 `beamlet::Authority` 与 `beamlet::Worker` 提供. 原 Agent crate 保留兼容导出, 不复制实现. 迁移独立角色消息流程测试.
- 验收: 仅依赖 beamlet 的调用者可以显式构造两种角色, 交付授权并收录结果; 重复投递, 等待取消, 错误绑定和乱序返回维持既有语义; 原 Agent 组合入口仍可用.
- 提交: `beamlet: move authority and worker drivers into library`.

### T2. 整理本地组合入口

状态: 待实现; 依赖 T1.

- 范围: 将旧 `Agent` 的本地执行组合迁入 beamlet, 使用不会与领域 Agent 混淆的名称. 提供显式组装入口, 保留迁移所需的兼容入口并更新已有调用点.
- 验收: 独立角色与可选组合入口使用同一实现; 接纳后派发失败仍保留原接纳事实; 取消等待不会重跑或取消已开始执行; 新入口不隐式建立基础设施.
- 提交: `beamlet: expose explicit local execution composition`.

### T3. 统一影响字段的领域命名

状态: 待实现; 在 T2 后执行, 避免与移动文件混为一个 diff.

- 范围: 按已确认设计将领域类型 `Effect` 调整为 `ImpactStatus`, 字段使用 `impact`, 更新库调用点. 当前 v0 JSON 的兼容映射在 Adapter 内明确保留, 不将 Rust 字段更名直接变成未声明的协议变化.
- 验收: 无影响/已观察/未知与成功/失败独立; 旧 v0 消息仍按其既有编码往返; 拒绝与证据校验语义不变.
- 提交: `kernel: name external impact evidence explicitly`.

### T4. 下沉按提交键接纳的通用能力

状态: 待实现; 依赖 T1.

- 范围: 从已有固定任务提交路径提炼 SubmissionKey 到 Operation/Attempt 的关联规则, 放入通用执行库; Authority 在接纳时分配执行身份. 保持现有协议 Adapter 的输入/输出, 让其调用通用接纳实现.
- 验收: 同 Authority 同键同内容返回原执行, 同键异内容拒绝, 相同内容不同键产生不同执行; 接纳失败不留下半份键映射; 核心逻辑无需 Zenoh 或应用 crate.
- 提交: `beamlet: centralize keyed operation admission`.

### T5. 提供本地 Client 提交与查询

状态: 待实现; 依赖 T4.

- 范围: 通过显式本地绑定实现提交, 原提交键查询, Operation 查询及 Report 查询. 确定最小结构化返回与错误, 不在同一提交加入完整远端订阅或持久恢复.
- 验收: 库调用者通过 Client 完成提交和结果查询; 回执丢失可按原键找回执行; 未知引用与冲突明确区分; 查询不创建执行; 应用不组装底层授权和信封.
- 提交: `beamlet: add local execution client queries`.

## 后续拆分时点

T5 完成后再根据真实接口细拆下一批提交: 结果观察与查询补齐, Zenoh Client 接入及 Console 调用点, 原授权/原 Report 交付驱动, 生命周期观察与 Node 显式组装. 提交重投, Attempt 重投, Report 重投和生命周期迁移分别验收, 不合成一次全量搬迁.

执行底座可独立使用后, 依次细拆存储事务与恢复, Agent 数据与 Loop, 回答查询与通知, 输入投影与 fork, 工具/Codemode 集成. 每项都沿用同一提交规则. 不提前实现 Sub-Agent, MCP Sampling 或跨节点自动接管.

进入存储阶段前对齐后端选择: 当前 workspace 已声明 `kite_sql`, `beamlet-durable` 仍为空依赖脚手架, 设计文档仍写 Diesel + SQLite. 新依赖声明尚不等于持久化契约已经实现, 当前五项任务不修改后端选择.

## 验证约定

Rust 改动运行格式检查, 相关库与调用方测试, Clippy 和库文档检查, 并完成仓库要求的检查. 验证范围随实际影响确定; 修改代码或出现失败后重跑相应检查. 提交说明记录结果与未实现范围, 不将文件迁移描述为新增可靠交付或持久恢复能力.
