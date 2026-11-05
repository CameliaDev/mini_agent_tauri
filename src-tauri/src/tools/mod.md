# `mod.rs`（tools 模块）

- 源码：[`mod.rs`](./mod.rs)
- 分层：工具层
- 拆分来源：原 `tools/mod.rs` 的 trait 与注册表部分（两个内置工具实现已移到 [`builtin/`](./builtin/mod.md)）

## 职责

定义所有工具都要实现的统一接口，以及按名字注册、收集说明书、分发执行的注册表。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod builtin` | 内置工具子模块 |
| `pub trait Tool` | `#[async_trait]`、`Send + Sync`，含 `name()`、`definition()`、`execute()` 三个方法 |
| `pub struct ToolRegistry` | 持有 `HashMap<String, Box<dyn Tool>>`，键是工具名 |
| `ToolRegistry::builtin()` | 注册 `get_current_time` 与 `echo` |
| `ToolRegistry::definitions()` | 遍历所有工具，收集 `Vec<ToolDefinition>` 作为请求体的 `tools` 字段 |
| `ToolRegistry::execute()` | 按模型给的名字找到工具、解析参数并执行 |

## 逻辑要点

1. 注册时写的字符串键必须和 `Tool::name()` 返回值一致，否则模型点名时查不到。
2. `execute(&ToolCall)` 先 `get(&call.function.name)`，查不到就返回 `未知工具: {name}` 错误。
3. `call.function.arguments` 是 **JSON 格式的字符串**，用 `serde_json::from_str` 解析；解析失败时用 `unwrap_or(Value::Null)` 降级成空值，再交给工具。
4. `Box<dyn Tool>` 让不同工具实现能放在同一张表里；`#[async_trait]` 让 trait 里可以写 `async fn`。

## 依赖关系

- 依赖：[`protocol`](../protocol.md) 的 `ToolCall`、`ToolDefinition`，以及具体的内置工具实现。
- 被谁使用：[`agent/runner.rs`](../agent/runner.md)（创建注册表、取说明书、执行调用）。

## 当前行为说明

- 参数 JSON 解析失败会静默降级为 `Null`，模型拿不到"参数格式错了"这个信息；也没有按 JSON Schema 做参数校验。
- 注册表在每次 `run_agent` 时新建，不持有跨轮状态。
