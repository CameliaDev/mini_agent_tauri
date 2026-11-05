# `settings.json`

- 源码：`settings.json`（本机文件，已被忽略，不在仓库中）
- 分层：编辑器本机设置（**已被 .gitignore 排除，不入库**）

## 职责

当前内容是 Python 分析器的本机配置：`python.analysis.extraPaths`、`stubPath`、`diagnosticSeverityOverrides`，指向 `C:\Users\...\k230_canmv_stubs\...` 这类绝对路径。

## 为什么单独说明它

它和本项目（Rust + TypeScript）**没有关系**，是别的嵌入式工程残留的 IDE 配置；而且路径只对当前这台机器有效。根目录的 [`.gitignore`](../.gitignore.md) 用 `.vscode/*` 把它排除、只白名单放行 [`extensions.json`](./extensions.md)。

> 这里刻意不给 `settings.json` 做文件链接：它不入库，做成链接在 clone 后的仓库里就是断链。文档里提到「不入库的文件」时，用行内代码而非链接。

## 依赖关系

- 相关：[`.gitignore.md`](../.gitignore.md)、[`extensions.md`](./extensions.md)。

## 当前行为说明

- 文件保留在本机、不删除（删除属于改动用户环境），但不会进入版本库；克隆仓库的人不会看到它。
- 如果要在团队里共享 Rust/TS 相关的编辑器设置，应新建一份不含绝对路径的配置并显式加入白名单。
