# `write.rs`

- 源码：[`write.rs`](./write.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `write` 工具：把 `contents` 覆盖写入工作区内的一个文件。真正落盘交给 [`atomic_write`](../sandbox.md)。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `WriteFile` | 持有工作区根的工具 |
| `impl Tool for WriteFile` | `name()` 返回 `"write"` |

## 逻辑要点

1. `path` 必填，会去掉首尾空白。`contents` 必填，但允许空字符串，用来清空文件，不会 `trim`。
2. 路径先过 [`resolve_inside`](../sandbox.md)。目标如果是目录，报错且不写。
3. 父目录必须已经存在。缺少中间目录时返回「父目录不存在」，并且不会把那个目录创建出来。
4. 成功文案区分「已写入」和「已覆盖写入」，并带上字节数（不是字符数）。
5. 逃出工作区的路径在写临时文件之前就失败，外部目录里不会出现新文件。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `writes_new_file_and_overwrite` | 新建、覆盖、空内容、UTF-8、不留临时文件 |
| `rejects_missing_parent_and_directory` | 父目录缺失时不创建；拒绝目录；缺 `contents` |
| `rejects_escape_without_creating_outside` | `../` 到另一个临时目录不会写出 `pwned.txt` |

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`sandbox`](../sandbox.md)、[`protocol`](../../protocol.md)。
- 被谁使用：[`ToolRegistry::with_workspace()`](../mod.md)。

## 当前行为说明

- 覆盖是整文件替换，没有补丁语义。改几行用 [`edit`](./edit.md)。
- 原子性的平台差异写在 [`sandbox.md`](../sandbox.md)：Windows 上替换已存在文件不是严格原子的。
