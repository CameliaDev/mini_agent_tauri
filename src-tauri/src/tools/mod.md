# `mod.rs`（tools 模块）

- 源码：[`mod.rs`](./mod.rs)
- 分层：工具层
- 拆分来源：原 `tools/mod.rs` 的 trait 与注册表部分（具体工具在 [`builtin/`](./builtin/mod.md)，路径边界在 [`sandbox.md`](./sandbox.md)）

## 职责

定义所有工具都要实现的统一接口，以及按名字注册、收集说明书、分发执行的注册表。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod builtin` | 内置工具子模块 |
| `pub mod sandbox` | 工作区根、路径逃逸检查、原子写入，见 [`sandbox.md`](./sandbox.md) |
| `pub trait Tool` | `#[async_trait]`、`Send + Sync`，含 `name()`、`definition()`、`execute()` 三个方法 |
| `pub struct ToolRegistry` | 持有 `HashMap<String, Box<dyn Tool>>`，键是工具名 |
| `ToolRegistry::builtin()` | 用 `load_workspace_root()` 当根，注册全部内置工具。没有取消信号 |
| `ToolRegistry::with_workspace()` | 用调用方给的根注册工具，取消信号为 `None`。测试走这里 |
| `ToolRegistry::with_workspace_and_cancel()` | 根目录和 `CancelSignal` 一起传入。`run_agent` 走这里，bash 才能在停止时杀进程 |
| `register()` | 用 `tool.name()` 当键插入，避免手写的字符串和 `name()` 对不上 |
| `ToolRegistry::definitions()` | 遍历所有工具，收集 `Vec<ToolDefinition>` 作为请求体的 `tools` 字段 |
| `ToolRegistry::execute()` | 按模型给的名字找到工具、解析参数并执行 |

## 逻辑要点

1. 键由 `register()` 取 `Tool::name()`，不再手写一遍。模型和注册表看到的是同一个名字。
2. `execute(&ToolCall)` 先 `get(&call.function.name)`，查不到就返回 `未知工具: {name}` 错误。
3. `call.function.arguments` 是 **JSON 格式的字符串**，用 `serde_json::from_str` 解析；解析失败时用 `unwrap_or(Value::Null)` 降级成空值，再交给工具。
4. `Box<dyn Tool>` 让不同工具实现能放在同一张表里；`#[async_trait]` 让 trait 里可以写 `async fn`。
5. `Tool::execute` 仍然只有参数，没有取消信号。需要它的只有 bash，所以信号放在 `Bash` 结构体里，不改 trait。

## 依赖关系

- 依赖：[`protocol`](../protocol.md) 的 `ToolCall`、`ToolDefinition`，[`sandbox`](./sandbox.md)，[`builtin/`](./builtin/mod.md) 里的具体工具，以及 [`CancelSignal`](../agent/cancel.md)（只为了交给 bash）。
- 被谁使用：[`agent/runner.rs`](../agent/runner.md)（`with_workspace_and_cancel` 创建注册表、取说明书、执行调用）。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `registers_builtin_tools` | 七个名字都在；`read` 的 `path` 必填，`bash` 的 `command` 必填 |
| `executes_read_by_name` | 按模型点名的字符串能执行到 `read`，临时工作区里的文件能读回来 |

## 当前行为说明

- 参数 JSON 解析失败会静默降级为 `Null`，模型拿不到"参数格式错了"这个信息；也没有按 JSON Schema 做参数校验。文件工具和 bash 会把缺参数报成 `Err`。
- 注册表在每次 `run_agent` 时新建，不持有跨轮状态。工作区根是构造时传入的，循环中途不会改。
- `with_workspace` / `builtin` 注册的 bash 没有取消信号。应用路径必须用 `with_workspace_and_cancel`，停止按钮才能杀掉正在跑的命令。
- `definitions()` 的顺序跟着 `HashMap` 走，不稳定。模型只按名字点工具，不依赖顺序。
