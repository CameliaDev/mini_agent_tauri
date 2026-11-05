# `mod.rs`（app 模块）

- 源码：[`mod.rs`](./mod.rs)
- 分层：Tauri 宿主层的模块入口

## 职责

把与 Tauri 宿主相关的代码收拢到 `app` 命名空间，使 [`agent`](../agent/mod.md)、[`providers`](../providers/mod.md) 等层不依赖具体的前端框架。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod commands` | 前端可调用的 Tauri 命令与事件转发，见 [`commands.md`](./commands.md) |
| `pub mod state` | 全局共享状态与配置加载，见 [`state.md`](./state.md) |

## 当前行为说明

这里只有模块声明，没有任何逻辑。
