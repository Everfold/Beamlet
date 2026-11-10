# Beamlet 执行底座

状态：提纲，正文、IDL 与模型待编写。

## 本节阅读顺序

1. [Operation](operation.md)：执行请求、Capability 契约、提交身份与 Authority 的记录阶段。
2. [Attempt](attempt.md)：执行授权、目标绑定与 Worker 的执行阶段。
3. [Report](report.md)：执行证据、结果、收录与采用。
4. [执行角色](roles.md)：Client、Authority、Worker 的职责、数据、能力与实例边界。
5. [执行交互](interaction.md)：Client、Authority、Worker 之间的完整交互流程。
6. [执行组合](composition.md)：父子执行与责任边界。

三个原语文档分别定义自身规则；角色文档集中描述参与方的契约；执行交互文档串联跨原语的流程，并引用相应规则。

## 核心原语关系图

待使用 Mermaid 绘制。

## 数据、接口与模型

| 文件 | 计划内容 |
| --- | --- |
| [data.proto](data.proto) | 共享数据与消息定义 |
| [interfaces.wit](interfaces.wit) | 组件接口与类型映射 |
| [模型导读](models/README.md) | 模型范围、假设与检查记录 |

## 后续阅读

[Beamlet-Agent](../beamlet-agent/README.md) · [验收案例](../conformance/beamlet.md) · [规范总导读](../README.md)
