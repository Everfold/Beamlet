# Beamlet Spec

本目录用于整理面向多语言实现的 Beamlet / Beamlet-Agent 规范。

**状态：目录与提纲已建立，具体内容待编写。**

本目录目前包含阅读导航、章节提纲和 IDL / Quint 占位文件；尚未迁入设计正文、定义具体接口或执行形式化检查。

正文、标题、表格、示例说明与注释统一使用中文，保留英文专有名词和程序标识符。文件路径沿用已确认的英文命名。

所有图表必须使用 Mermaid，并以 `mermaid` 代码块编写，详见[图表约定](conventions.md#图表)。

## 如何阅读

按理解问题的顺序阅读；同一主题的说明、数据定义、接口和模型放在一起。

| 顺序 | 主题 | 回答的问题 |
| --- | --- | --- |
| 1 | [总览](overview/README.md) | Beamlet 解决什么问题？有哪些概念、角色和边界？ |
| 2 | [Beamlet](beamlet/README.md) | Operation 如何提交、获得授权、执行并返回 Report？ |
| 3 | [Beamlet-Agent](beamlet-agent/README.md) | Agent 如何使用执行底座，处理输入、调用和结果？ |
| 4 | [环境适配](bindings/README.md) | 领域契约如何映射到本地调用、消息传输和存储？ |
| 5 | [实现验收](conformance/README.md) | 如何检查各语言实现是否满足规范要求？ |

[规范写作与审阅约定](conventions.md)供编写者和审阅者查阅；首次理解项目时可以先从总览开始。

## 目录

- `spec/`

  - [README.md](README.md)：阅读导航、规范状态
  - [conventions.md](conventions.md)：规范写法、要求编号、IDL 使用与映射约定
  - `overview/`

    - [README.md](overview/README.md)：目标、范围与完整执行示例
    - [glossary.md](overview/glossary.md)：统一术语
    - [architecture.md](overview/architecture.md)：角色、组件依赖和部署关系

  - `beamlet/`

    - [README.md](beamlet/README.md)：执行底座导读
    - [operation.md](beamlet/operation.md)：执行请求、Capability 契约、提交身份与 Authority 的记录阶段
    - [attempt.md](beamlet/attempt.md)：执行授权、目标绑定与 Worker 的执行阶段
    - [report.md](beamlet/report.md)：执行证据、结果、收录与采用
    - [roles.md](beamlet/roles.md)：角色职责、数据、能力、运行实例与接管边界
    - [interaction.md](beamlet/interaction.md)：提交、回执、查询、观察及错误语义
    - [composition.md](beamlet/composition.md)：父子执行关系与责任边界
    - [data.proto](beamlet/data.proto)：本主题共享数据定义
    - [interfaces.wit](beamlet/interfaces.wit)：本主题组件接口及共享类型映射
    - `models/`

      - [README.md](beamlet/models/README.md)：假设、覆盖的要求和实际检查结果
      - [execution.qnt](beamlet/models/execution.qnt)：关键执行行为的抽象模型

  - `beamlet-agent/`

    - [README.md](beamlet-agent/README.md)：Agent 导读及其与 Beamlet 的关系
    - [agent.md](beamlet-agent/agent.md)：Agent 身份、配置、历史、恢复记录与 Fork
    - [loop.md](beamlet-agent/loop.md)：Loop 行为、状态、调用、等待、完成与恢复推进
    - [interaction.md](beamlet-agent/interaction.md)：Input 提交、Output 观察与控制请求的接口边界
    - [data.proto](beamlet-agent/data.proto)：Agent 数据定义，引用 Beamlet 公共类型
    - [interfaces.wit](beamlet-agent/interfaces.wit)：Agent 对外接口与宿主依赖
    - `models/`

      - [README.md](beamlet-agent/models/README.md)：假设、覆盖的要求和实际检查结果
      - [loop.qnt](beamlet-agent/models/loop.qnt)：关键 Loop 行为的抽象模型

  - `bindings/`

    - [README.md](bindings/README.md)：适配范围与选择方式
    - [local.md](bindings/local.md)：同进程调用的映射
    - [messaging.md](bindings/messaging.md)：消息传输的映射
    - [storage.md](bindings/storage.md)：数据保存、原子更新、记录加载与版本迁移

  - `conformance/`

    - [README.md](conformance/README.md)：实现验收方式
    - [beamlet.md](conformance/beamlet.md)：跨语言执行行为案例
    - [beamlet-agent.md](conformance/beamlet-agent.md)：跨语言 Agent 行为案例

环境适配只在需要约定适配方式时细化；目录存在不代表必须接入某种基础设施。存储规范的整理不改变“暂不实现持久化”的范围。

## 后续填充与审阅顺序

1. 阅读主题的 README 和 Markdown 行为说明，理解目标、前置条件、状态变化和保证边界。
2. 沿文中链接检查 `.proto` 的消息与字段、`.wit` 的接口与错误返回。
3. 对照模型说明，检查 `.qnt` 是否表达了指定要求，以及模型采用了哪些假设。
4. 对照实现验收中的行为案例，检查实现是否给出要求的可观察结果。

各主题 README 负责导航和简短流程示例，不重复维护一整份规范。行为要求使用稳定编号，由数据、接口、模型和验收案例引用；未决问题明确标出，不能默认为已确认要求。

## 规范写作与审阅约定

后续在 [conventions.md](conventions.md) 中集中确定：

- 规范性要求与示例、设计理由的区别，以及文档状态和要求编号。
- Markdown、Protobuf、WIT、Quint 各自的定义责任和互相引用规则。
- 两种 IDL 的采用范围，以及跨 IDL 类型、字段存在性和错误的映射。
- 模型的范围、假设、性质与实际检查记录。

当前仅建立这些约定的提纲，不新增领域规则。
