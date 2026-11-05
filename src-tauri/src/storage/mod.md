# `storage/mod.rs`

- 源码：[`mod.rs`](./mod.rs)
- 分层：持久化层

## 职责

把会话和消息存进本地 SQLite，让「刷新页面 / 重启程序」不丢东西。

## 为什么放在后端而不是前端

前端的 `localStorage` 也能存，但桌面应用的数据应该跟应用放在一起：卸载重装时一起清掉，而且 Tauri 提供了标准的「应用数据目录」路径（`app.path().app_data_dir()`）。

## 为什么用 SQLite 而不是 JSON 文件

1. **写入是事务性的**：中途失败不会留下半截数据；
2. **查询是结构化的**：要「取某个会话的全部消息」不需要把整个文件读进内存；
3. **rusqlite 是同步 API**，但我们的命令都是短操作，用 `Mutex` 包一层连接就够了。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `struct Storage` | 存储句柄：`Arc<Mutex<Connection>>`。`Arc` 让句柄可以廉价克隆，`Mutex` 保证同一时刻只有一个任务在用连接 |
| `fn open(PathBuf)` | 打开（或创建）数据库文件，自动建表 |
| `fn open_in_memory()` | 打开内存数据库，测试专用 |
| `fn list_sessions()` | 列出全部会话，按更新时间倒序 |
| `fn load_messages(session_id)` | 读某个会话的全部消息，按 `seq` 升序 |
| `fn upsert_session(&SessionMeta)` | 新建或覆盖一个会话 |
| `fn append_message(session_id, seq, &StoredMessage)` | 追加一条消息 |
| `fn delete_message(session_id, message_id)` | 删一条消息 |
| `fn delete_session(session_id)` | 删会话（外键级联把消息一起删） |
| `fn clear_messages(session_id)` | 清空消息但保留会话 |

## 逻辑要点

1. **所有 SQL 都用参数绑定**（`params![]`），不做字符串拼接——这是防 SQL 注入的第一道防线。
2. `INSERT OR REPLACE` 按消息 id 覆盖：前端重试或重落库时不会产生重复行。
3. `tool_call` 是 `Option<Value>`，落库时序列化成 JSON 文本，读出来再 `serde_json::from_str`；解析失败就当没有，**不能因为一条脏数据整个会话读不出来**。
4. `seq` 由调用方给（前端传数组下标），正好就是对话顺序；读取时 `ORDER BY seq ASC`。
5. `Connection` 是 `!Sync` 的，所以外面包了 `Mutex`；`Arc` 是因为 Tauri 的 `State` 不可变，命令之间要共享同一个连接。
6. 文件末尾有编译期断言：`_assert_send_sync::<Storage>()`，保证它满足 Tauri 托管状态的要求。

## 单元测试

`cd src-tauri && cargo test`，本文件 7 个测试，全部用内存库，不需要真实文件：

| 测试 | 验证什么 |
| --- | --- |
| `saves_and_lists_sessions` | 会话写进去能读出来 |
| `upsert_replaces_by_id` | 同 id 重复存是覆盖而不是新增 |
| `loads_messages_in_order` | 乱序写入后按 `seq` 读出来仍是对话顺序 |
| `deletes_session_with_messages` | 删会话时消息随外键级联一起删 |
| `deletes_single_message` | 删单条消息 |
| `clears_messages_but_keeps_session` | 清空消息但保留会话 |
| `round_trips_tool_call` | 工具调用能原样存进去再读出来 |

## 依赖关系

- 依赖：`rusqlite`、`tokio::sync::Mutex`、`crate::protocol` 的 `SessionMeta` / `StoredMessage`、[`schema.rs`](./schema.md)。
- 被谁使用：[`app/commands.rs`](../app/commands.md)（对外命令）、[`lib.rs`](../lib.md)（`setup` 里打开数据库并 `manage`）。

## 当前行为说明

- 每次落库是「先 upsert 会话，再逐条 append 消息」，没有做批量事务；消息多时会有点慢，但逻辑简单不容易错。
- 没有做全文搜索、消息分页；长对话目前是整段读出来。
- 数据库文件固定叫 `mini_agent.db`，放在系统应用数据目录下。
