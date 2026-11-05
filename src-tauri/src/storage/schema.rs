// ============================================================================
// 数据库表结构与初始化
// 单独一个文件：schema 是「数据长什么样」的定义，和「怎么读写」分开更清楚
// ============================================================================

// 引入 rusqlite 的连接类型
use rusqlite::Connection;

// 定义当前 schema 版本号
// 以后要加列就 +1，然后在 init 里写对应的 ALTER TABLE
pub const SCHEMA_VERSION: i64 = 1;

// 初始化数据库：建表 + 设置 PRAGMA
// 用 CREATE TABLE IF NOT EXISTS，所以重复调用是安全的
pub fn init(conn: &Connection) -> anyhow::Result<()> {
    // 用一个批次执行建表语句
    // 外键开关：SQLite 默认关闭，不开的话删会话不会自动删消息
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         -- 会话表
         CREATE TABLE IF NOT EXISTS sessions (
             id         TEXT PRIMARY KEY,
             title      TEXT NOT NULL,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL
         );
         -- 消息表：session_id 是外键，指向 sessions.id
         CREATE TABLE IF NOT EXISTS messages (
             id         TEXT PRIMARY KEY,
             session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
             seq        INTEGER NOT NULL,
             role       TEXT NOT NULL,
             content    TEXT NOT NULL DEFAULT '',
             tool_name  TEXT,
             tool_call  TEXT,
             created_at INTEGER NOT NULL
         );
         -- 给「按会话查消息」建索引：否则每条消息都要全表扫
         CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, seq);",
    )?;
    // 记录 schema 版本（PRAGMA user_version 是 SQLite 内置的整数槽位，不用自己建表）
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    // 返回成功
    Ok(())
}
