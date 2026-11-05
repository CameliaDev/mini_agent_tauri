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
| `pub mod read` | 按行读取工作区文本，见 [`read.md`](./read.md) |
| `pub mod write` | 原子写入工作区文件，见 [`write.md`](./write.md) |
| `pub mod edit` | 唯一匹配替换，见 [`edit.md`](./edit.md) |
| `pub mod list_dir` | 列出一层目录，见 [`list_dir.md`](./list_dir.md) |
| `pub mod bash` | 在工作区里执行命令，见 [`bash.md`](./bash.md) |

## 依赖关系

- 依赖：无（只有模块声明）。
- 被谁使用：[`tools/mod.rs`](../mod.md) 的 `ToolRegistry::with_workspace()` / `with_workspace_and_cancel()` 从这里取具体类型来注册。

## 当前行为说明

文件工具都要一个工作区根，根的检查在 [`sandbox.rs`](../sandbox.md)，不在这个文件里。`bash` 也用这个根当当前目录，另外还要一份取消信号。这里仍然只做 `mod` 声明。
