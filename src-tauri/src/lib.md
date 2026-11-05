# `lib.rs`

- 源码：[`lib.rs`](./lib.rs)
- 分层：crate 根 / Tauri 应用装配
- 拆分来源：原来的 `AppState`、`greet`、`chat`、`run_agent`、配置加载都曾写在这里

## 职责

整个库的总入口，只负责三件事：声明模块、导入命令、装配并启动 Tauri。拆分之后这里不再包含任何 agent 业务逻辑。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod agent` | agent 运行时层（事件、会话、主循环） |
| `pub mod app` | Tauri 宿主层（状态、命令） |
| `pub mod protocol` | provider 无关的数据结构 |
| `pub mod providers` | 模型适配层 |
| `pub mod tools` | 工具层 |
| `use app::commands::{chat, greet}` | 把两个命令引入当前作用域，供宏注册 |
| `pub fn run()` | 装配 Tauri Builder 并启动应用 |

## 逻辑要点

### setup 钩子：打开数据库

```rust
.setup(|app| {
    let dir = app.path().app_data_dir()?;      // 系统应用数据目录
    let db_path = dir.join("mini_agent.db");
    let storage = tauri::async_runtime::block_on(Storage::open(db_path))?;
    app.manage(storage);                       // 注册成全局状态
    Ok(())
})
```

- `setup` 在窗口创建前跑一次，是「需要一个真实路径才能初始化资源」的标准位置。
- `app.path()` 和 `app.manage()` 都来自 `tauri::Manager` trait，别忘了 `use tauri::Manager;`。
- `Storage::open` 是 async，但 setup 阶段没有别的任务在跑，用 `block_on` 同步等它完成即可。

### 注册的命令

| 命令 | 作用 |
| --- | --- |
| `greet` | 模板自带的示例 |
| `chat` | 跑一轮 agent，流式推事件 |
| `stop_generation` | 打断正在进行的生成 |
| `list_sessions` / `load_messages` / `save_session` / `append_message` | 会话与消息的读写 |
| `delete_session` / `delete_session_messages` / `clear_session_messages` | 删除与清空 |


1. `tauri::Builder::default()` 创建应用构建器。
2. `.plugin(tauri_plugin_opener::init())` 注册打开外部链接的插件。
3. `.manage(AppState::from_env())` 把配置注册为全局状态；密钥读取与 `.env` 加载都在 [`app/state.rs`](./app/state.rs) 里完成。
4. `.invoke_handler(tauri::generate_handler![greet, chat])` 注册两个前端可调用的命令。
5. `.run(tauri::generate_context!())` 启动事件循环，失败则 panic 并打印提示。

## 依赖关系

- 依赖：Tauri、`tauri-plugin-opener`、`app::commands`、`app::state`。
- 被谁使用：[`main.rs`](./main.rs) 的 `main()`。

## 当前行为说明

`greet` 是模板示例命令，代码注释里写明"保留不动"；`chat` 是真正跑 agent 的命令。
