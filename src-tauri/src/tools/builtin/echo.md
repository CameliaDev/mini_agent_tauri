# `echo.rs`

- 源码：[`echo.rs`](./echo.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `echo` 工具：把模型传进来的文本原样返回，主要用来验证「带参数的工具调用」整条链路是否通畅。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub struct Echo` | 无字段的工具单元结构体 |
| `impl Tool for Echo` | 实现 `name()`、`definition()`、`execute()` |

## 逻辑要点

1. `name()` 返回 `"echo"`，与 [`ToolRegistry::builtin()`](../mod.md) 注册时的键一致。
2. `definition()` 声明一个必填的字符串参数 `text`，并在 `required` 里列出 `"text"`，让模型知道要生成什么形状的 JSON 参数。
3. `execute()` 用 `args["text"].as_str()` 取值；取不到时用 `unwrap_or("(没有收到 text 参数)")` 给一段提示文本，仍然返回 `Ok`。

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`protocol`](../../protocol.md) 的 `ToolDefinition` / `ToolFunction`、`serde_json::Value`。
- 被谁使用：[`ToolRegistry::builtin()`](../mod.md) 注册后，由模型点名调用。

## 当前行为说明

参数缺失或类型不对时不会报错，而是返回提示文本；因此模型看到的是一次"成功"的工具结果。
