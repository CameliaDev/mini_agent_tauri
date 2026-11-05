# `mod.rs`（builtin 模块）

- 源码：[`mod.rs`](./mod.rs)
- 分层：工具层 / 内置工具入口

## 职责

把随应用自带的工具挂在 `tools::builtin` 命名空间下，每个工具单独一个文件，方便以后增删。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod time` | 获取当前本地时间，见 [`time.md`](./time.md) |
| `pub mod echo` | 回显传入文本，见 [`echo.md`](./echo.md) |

## 依赖关系

- 依赖：无（只有模块声明）。
- 被谁使用：[`tools/mod.rs`](../mod.md) 的 `ToolRegistry::builtin()` 从这里取具体类型来注册。

## 当前行为说明

目前只有两个示例级工具，都不涉及文件、网络或系统操作。
