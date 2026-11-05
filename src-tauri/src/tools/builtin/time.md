# `time.rs`

- 源码：[`time.rs`](./time.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `get_current_time` 工具：不需要参数，返回格式化后的当前本地日期时间。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub struct GetCurrentTime` | 无字段的工具单元结构体 |
| `impl Tool for GetCurrentTime` | 实现 `name()`、`definition()`、`execute()` |

## 逻辑要点

1. `name()` 返回 `"get_current_time"`，和 [`ToolRegistry::builtin()`](../mod.md) 里插入的键保持一致。
2. `definition()` 构造 `ToolDefinition`：`kind` 固定 `"function"`，`description` 写明不需要参数，`parameters` 是 `{"type":"object","properties":{},"required":[]}`。
3. `execute()` 忽略参数，用 `chrono::Local::now().format("%Y-%m-%d %H:%M:%S")` 生成字符串并包成 `Ok(...)`。

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`protocol`](../../protocol.md) 的 `ToolDefinition` / `ToolFunction`、`chrono`。
- 被谁使用：[`ToolRegistry::builtin()`](../mod.md) 注册后，由模型点名调用。

## 当前行为说明

取的是运行应用的机器本地时间，格式不含时区信息；`execute()` 是 `async fn`，但内部没有异步操作。
