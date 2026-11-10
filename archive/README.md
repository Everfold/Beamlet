# 历史文档归档

这里保存旧设计、实施计划和实现记录，供查阅决策背景。现行规范与审阅入口位于 [spec](../spec/README.md)。归档中的“当前”“已实现”“后续”等描述均属于归档时的语境，不能据此确定现行规范或代码状态。

## 2026-11-11：建立跨语言规范前的快照

- 目录：[2026-11-11-pre-spec](2026-11-11-pre-spec/README.md)。该链接展示的是归档前的项目 README。
- 来源提交：`5a70b1fefdfce001de89b078338b26e22ef2e23a`。
- 范围：根 `GLOSSARY.md`、`spec/design/` 下的全部 12 份 Markdown 文档，以及根 `README.md` 的副本。
- 原文按归档前的仓库相对路径保存，文件内容逐字节不变；根 README 继续保留在项目入口。
- 校验清单：[SHA256SUMS](2026-11-11-pre-spec/SHA256SUMS)，路径相对于快照目录。
- 技能、工具指令、许可证和源代码注释不属于此次设计文档归档范围。

### 历史阅读入口

- [设计总览](2026-11-11-pre-spec/spec/design/README.md)
- [术语表](2026-11-11-pre-spec/GLOSSARY.md)
- [Agent 实体与状态](2026-11-11-pre-spec/spec/design/agent-model.md)
- [实现计划](2026-11-11-pre-spec/spec/design/implementation-plan.md)
- [实现任务](2026-11-11-pre-spec/spec/design/implementation-tasks.md)
- [架构审阅修复](2026-11-11-pre-spec/spec/design/architecture-fixes.md)

归档内部的文档相对链接保持可用。原文指向代码或配置的相对链接保留原样；快照不包含代码，查阅这些链接时应以来源提交中的原仓库路径为准。

### 校验归档内容

在仓库根目录执行：

```bash
(cd archive/2026-11-11-pre-spec && sha256sum -c SHA256SUMS)
```
