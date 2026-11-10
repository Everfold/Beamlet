# Beamlet-Agent

状态：提纲，正文、IDL 与模型待编写。

## 本节阅读顺序

1. [Agent 实体](agent.md)：保存的数据及其关系，包括身份、配置、历史、恢复记录与 Fork。
2. [Agent Loop](loop.md)：处理数据的时机与状态变化，包括调用、等待、完成和恢复推进。
3. [Agent 交互](interaction.md)：外部提交 Input、观察 Output 和请求控制的接口边界。

文档以 Agent 这一核心实体组织。Input / Output 属于交互记录，Loop 描述行为模型，Preset / Override 属于配置模型，Fork 是 Agent 的操作。

Loop、LLM Request、Codemode 及映射为独立执行的 Tool 调用复用 Beamlet 的 Operation / Attempt / Report。

## 数据、接口与模型

| 文件 | 计划内容 |
| --- | --- |
| [data.proto](data.proto) | Agent 数据与消息定义 |
| [interfaces.wit](interfaces.wit) | Agent 接口与宿主依赖 |
| [模型导读](models/README.md) | 模型范围、假设与检查记录 |

## 相关阅读

[Beamlet 执行底座](../beamlet/README.md) · [验收案例](../conformance/beamlet-agent.md) · [规范总导读](../README.md)
