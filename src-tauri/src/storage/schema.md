# `schema.rs`

- 源码：[`schema.rs`](./schema.rs)
- 分层：持久化层 / 表结构

## 职责

定义「数据长什么样」：建表语句和 PRAGMA 设置。单独一个文件，是为了让「结构」和「读写」分开——改表结构时只动这里。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `const SCHEMA_VERSION` | 当前 schema 版本号，存进 SQLite 内置的 `PRAGMA user_version` 槽位 |
| `fn init(&Connection)` | 建表 + 设置 PRAGMA，重复调用安全 |

## 表结构

**sessions**（会话元信息）

| 列 | 类型 | 说明 |
| --- | --- | --- |
| `id` | TEXT PRIMARY KEY | 会话 id，前端生成的 UUID |
| `title` | TEXT NOT NULL | 会话标题，默认「新对话」，首条消息后自动改成摘要 |
| `created_at` | INTEGER NOT NULL | 创建时间（毫秒时间戳） |
| `updated_at` | INTEGER NOT NULL | 最后更新时间，侧栏按它排序和分组 |

**messages**（消息正文）

| 列 | 类型 | 说明 |
| --- | --- | --- |
| `id` | TEXT PRIMARY KEY | 消息 id |
| `session_id` | TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE | 所属会话，外键级联删除 |
| `seq` | INTEGER NOT NULL | 在会话里的第几条，决定显示顺序 |
| `role` | TEXT NOT NULL | `user` / `assistant` / `tool` |
| `content` | TEXT NOT NULL DEFAULT '' | 正文；工具消息这里是空串 |
| `tool_name` | TEXT | 工具名，只有 `role=tool` 有值 |
| `tool_call` | TEXT | 完整工具调用（JSON 文本），只有 `role=tool` 有值 |
| `created_at` | INTEGER NOT NULL | 创建时间（毫秒时间戳） |

## 逻辑要点

1. **建表用 `CREATE TABLE IF NOT EXISTS`**：重复调用是安全的，启动时不用判断表在不在。
2. **`PRAGMA foreign_keys = ON` 必须显式开**：SQLite 默认关闭外键约束，不开的话 `ON DELETE CASCADE` 根本不生效，删会话会留下孤儿消息。
3. **索引 `idx_messages_session` 建在 `(session_id, seq)` 上**：没有它，「取某个会话的消息」每次都要全表扫。
4. **`PRAGMA user_version` 是 SQLite 内置的整数槽位**，不用自己建表来记版本。
5. `execute_batch` 一次执行多条语句，比逐条 `execute` 快。

## 以后要加列怎么办

1. 把 `SCHEMA_VERSION` 加 1；
2. 在 `init()` 里写对应的 `ALTER TABLE ... ADD COLUMN`（注意 SQLite 的 `ADD COLUMN` 不能带非空默认值，要给默认值）；
3. `init()` 每次启动都会跑，所以迁移语句天然是幂等的。

## 依赖关系

- 依赖：`rusqlite::Connection`。
- 被谁使用：[`storage/mod.rs`](./mod.md)。
