# Beamlet 规范

本目录用于整理面向人类审阅和多语言实现的 Beamlet / Beamlet-Agent 规范。Rust 是实现起点；规范描述跨语言的语义与组件契约。

**状态：目录结构提案。** 旧文档已完整[归档](../archive/README.md)。目前仅建立本阅读入口，下列主题目录与文件尚未编写，也不表示已经确认接口或完成形式化检查。

## 如何阅读

按理解问题的顺序阅读；同一主题的说明、数据定义、接口和模型放在一起。

| 顺序 | 主题 | 回答的问题 |
| --- | --- | --- |
| 1 | Overview | Beamlet 解决什么问题？有哪些概念、角色和边界？ |
| 2 | Beamlet | Operation 如何提交、获得授权、执行并返回 Report？ |
| 3 | Beamlet-Agent | Agent 如何使用执行底座，处理输入、调用和结果？ |
| 4 | Bindings | 领域契约如何映射到本地调用、消息传输和存储？ |
| 5 | Conformance | 一个实现如何证明自己满足指定的规范要求？ |

[规范写作与审阅约定](#规范写作与审阅约定)供编写者和审阅者查阅；首次理解项目时可以先从 Overview 开始。

## 建议目录

下面展示计划中的文件，不提前创建空文档。主题较短时保留在一个文件中，内容确实增多后再拆分。

```text
spec/
  README.md                       # 阅读导航、规范状态
  conventions.md                  # 规范写法、要求编号、IDL 使用与映射约定
  overview/
    README.md                     # 目标、范围与完整执行示例
    glossary.md                   # 统一术语
    architecture.md               # 角色、组件依赖和部署关系
  beamlet/
    README.md                     # 执行底座导读
    execution.md                  # Operation / Attempt / Report 与状态规则
    interaction.md                # 提交、回执、查询、观察及错误语义
    composition.md                # 父子执行关系与责任边界
    data.proto                    # 本主题共享数据定义
    interfaces.wit                # 本主题组件接口及共享类型映射
    models/
      README.md                   # 假设、覆盖的要求和实际检查结果
      execution.qnt               # 关键执行行为的抽象模型
  beamlet-agent/
    README.md                     # Agent 导读及其与 Beamlet 的关系
    entity.md                     # 身份、配置、数据关联与 fork
    loop.md                       # 推进、等待、调用组合与完成条件
    interaction.md                # 输入、输出、查询与观察
    data.proto                    # Agent 数据定义，引用 Beamlet 公共类型
    interfaces.wit                # Agent 对外接口与宿主依赖
    models/
      README.md                   # 假设、覆盖的要求和实际检查结果
      loop.qnt                    # 关键 Loop 行为的抽象模型
  bindings/
    README.md                     # 适配范围与选择方式
    local.md                      # 同进程调用的映射
    messaging.md                  # 消息传输的映射
    storage.md                    # 存储边界与确认语义
  conformance/
    README.md                     # 实现验收方式
    beamlet.md                    # 跨语言执行行为案例
    beamlet-agent.md               # 跨语言 Agent 行为案例
```

Bindings 只在需要约定适配方式时细化；目录存在不代表必须接入某种基础设施。存储规范的整理不改变“暂不实现持久化”的范围。实现进度和 Rust 开发任务不作为领域要求混入这些主题。

## 同一主题如何审阅

1. 阅读主题的 README 和 Markdown 行为说明，理解目标、前置条件、状态变化和保证边界。
2. 沿文中链接检查 `.proto` 的消息与字段、`.wit` 的接口与错误返回。
3. 对照模型说明，检查 `.qnt` 是否表达了指定要求，以及模型采用了哪些假设。
4. 对照 Conformance 中的行为案例，检查实现是否给出要求的可观察结果。

各主题 README 负责导航和简短流程示例，不重复维护一整份规范。行为要求使用稳定编号，由数据、接口、模型和验收案例引用；未决问题明确标出，不能默认为已确认要求。

## 规范写作与审阅约定

后续在 `conventions.md` 中集中确定：

- 规范性要求与示例、设计理由的区别，以及文档状态和要求编号。
- Markdown、Protobuf、WIT、Quint 各自的定义责任和互相引用规则。
- 两种 IDL 的采用范围，以及跨 IDL 类型、字段存在性和错误的映射。
- 模型的范围、假设、性质与实际检查记录。

当前目录仅提出这些约定的编写位置，不在迁移文档时新增领域规则。
