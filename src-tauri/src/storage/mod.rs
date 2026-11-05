// ============================================================================
// 这一层负责：把会话和消息存进本地 SQLite，让「刷新页面/重启程序」不丢东西
//
// 为什么放在后端而不是前端：
//   前端的 localStorage 也能存，但桌面应用的数据应该跟应用放在一起，
//   卸载重装时一起清掉，而且 Tauri 提供了标准的「应用数据目录」路径。
//
// 为什么用 SQLite（而不是 JSON 文件）：
//   1) 写入是事务性的：中途失败不会留下半截数据；
//   2) 查询是结构化的：要「取某个会话的全部消息」不需要把整个文件读进内存；
//   3) rusqlite 是同步 API，但我们的命令都是短操作，直接用 Mutex 包一层连接即可。
//
// 表结构（两张表）：
//   sessions —— 会话元信息：id / title / created_at / updated_at
//   messages —— 消息正文：id / session_id / role / content / tool_name / tool_call / seq
//   messages.session_id 上有外键，删会话时消息自动一起删（PRAGMA foreign_keys=ON）
// ============================================================================

// 声明子模块
pub mod schema;

// 引入 rusqlite 的参数绑定宏与连接类型
use rusqlite::{Connection, params};
// 引入 tokio 的互斥锁：SQLite 连接是 !Sync 的，用锁保证同一时刻只有一个任务在用
use tokio::sync::Mutex;
// 引入 Arc：让整个存储句柄可以被廉价克隆（多个命令共享同一个连接）
use std::sync::Arc;
// 引入协议层的持久化数据结构
use crate::protocol::{SessionMeta, StoredMessage};

// ============================================================================
// 对外的存储句柄
// ============================================================================

// 定义存储句柄：内部持有被 Mutex 保护住的数据库连接
// Clone 是必须的：Tauri 的 State 是不可变的，但命令之间要共享同一个连接
#[derive(Clone)]
pub struct Storage {
    // 连接本身；Arc + Mutex 保证并发安全，且句柄可以随意克隆
    conn: Arc<Mutex<Connection>>,
}

// 为 Storage 实现方法
impl Storage {
    // 打开（或创建）数据库文件，并保证表结构是最新的
    // path 一般传 Tauri 的 app_data_dir() 拼出来的路径
    pub async fn open(path: std::path::PathBuf) -> anyhow::Result<Self> {
        // 确保父目录存在；不存在就创建
        if let Some(parent) = path.parent() {
            // create_dir_all 是幂等的，目录已存在也不报错
            std::fs::create_dir_all(parent)?;
        }
        // 打开连接；文件不存在会被自动创建
        let conn = Connection::open(&path)?;
        // 立刻建表并设置 PRAGMA
        schema::init(&conn)?;
        // 包进 Mutex 后返回
        Ok(Self {
            // 锁住连接
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // 打开一个纯内存数据库：测试专用，不需要文件
    pub async fn open_in_memory() -> anyhow::Result<Self> {
        // 内存库的连接名是 ":memory:"
        let conn = Connection::open_in_memory()?;
        // 同样要建表
        schema::init(&conn)?;
        // 包起来
        Ok(Self {
            // 锁住连接
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // 列出全部会话，按更新时间倒序（新的在前）
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionMeta>> {
        // 拿锁；guard 在函数结束时自动释放
        let conn = self.conn.lock().await;
        // 准备语句：SELECT 一行会话元信息
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at FROM sessions ORDER BY updated_at DESC",
        )?;
        // 执行查询，把每一行映射成 SessionMeta
        let rows = stmt.query_map([], |row| {
            // 按列顺序读出来
            Ok(SessionMeta {
                // 第 1 列：id
                id: row.get(0)?,
                // 第 2 列：title
                title: row.get(1)?,
                // 第 3 列：创建时间
                created_at: row.get(2)?,
                // 第 4 列：更新时间
                updated_at: row.get(3)?,
            })
        })?;
        // 收集成 Vec；`?` 会把 rusqlite 错误转成 anyhow 错误
        let mut out = Vec::new();
        // 逐行读取
        for row in rows {
            // push 每一行
            out.push(row?);
        }
        // 返回列表
        Ok(out)
    }

    // 读出某个会话的全部消息（按 seq 升序，即对话顺序）
    pub async fn load_messages(&self, session_id: &str) -> anyhow::Result<Vec<StoredMessage>> {
        // 拿锁
        let conn = self.conn.lock().await;
        // 准备语句：WHERE session_id = ?1 用的是参数绑定，不是字符串拼接
        let mut stmt = conn.prepare(
            "SELECT id, role, content, tool_name, tool_call, created_at
             FROM messages WHERE session_id = ?1 ORDER BY seq ASC",
        )?;
        // 执行查询
        let rows = stmt.query_map(params![session_id], |row| {
            // tool_call 存的是 JSON 文本，读出来后再解析成 Value
            let tool_call_text: Option<String> = row.get(4)?;
            // 解析 JSON；解析失败就当没有（不能因为一条脏数据整个会话读不出来）
            let tool_call = tool_call_text.and_then(|t| serde_json::from_str(&t).ok());
            // 组装成 StoredMessage
            Ok(StoredMessage {
                // 第 1 列：消息 id
                id: row.get(0)?,
                // 第 2 列：角色
                role: row.get(1)?,
                // 第 3 列：正文
                content: row.get(2)?,
                // 第 4 列：工具名
                tool_name: row.get(3)?,
                // 解析好的工具调用
                tool_call,
                // 第 6 列：创建时间
                created_at: row.get(5)?,
            })
        })?;
        // 收集成 Vec
        let mut out = Vec::new();
        // 逐行读取
        for row in rows {
            // push 每一行
            out.push(row?);
        }
        // 返回消息列表
        Ok(out)
    }

    // 新建或更新一个会话（按 id 覆盖元信息，不动它的消息）
    pub async fn upsert_session(&self, meta: &SessionMeta) -> anyhow::Result<()> {
        // 拿锁
        let conn = self.conn.lock().await;
        // 必须原地 UPDATE，不能 INSERT OR REPLACE。
        // REPLACE 会先 DELETE 旧行；messages.session_id 是 ON DELETE CASCADE，
        // 这一下会把该会话的消息全部删掉。前端若正好在落库窗口里重读，
        // 就会拿空列表盖掉内存里刚发出的消息。
        conn.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title,
               created_at = excluded.created_at,
               updated_at = excluded.updated_at",
            params![meta.id, meta.title, meta.created_at, meta.updated_at],
        )?;
        // 返回成功
        Ok(())
    }

    // 追加一条消息（seq 由调用方给，代表它在会话里的第几条）
    pub async fn append_message(
        &self,
        session_id: &str,
        seq: i64,
        message: &StoredMessage,
    ) -> anyhow::Result<()> {
        // 拿锁
        let conn = self.conn.lock().await;
        // tool_call 是 Option<Value>，落库时序列化成 JSON 文本
        let tool_call_text = message
            .tool_call
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        // INSERT OR REPLACE：同 id 重复写入就替换，避免重试导致重复行
        conn.execute(
            "INSERT OR REPLACE INTO messages
             (id, session_id, seq, role, content, tool_name, tool_call, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                message.id,
                session_id,
                seq,
                message.role,
                message.content,
                message.tool_name,
                tool_call_text,
                message.created_at,
            ],
        )?;
        // 返回成功
        Ok(())
    }

    // 删除一个会话（外键级联会把它的消息一起删掉）
    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        // 拿锁
        let conn = self.conn.lock().await;
        // 删会话行
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
        // 返回成功
        Ok(())
    }

    // 删除某一条消息（重新生成时要把旧回复删掉）
    pub async fn delete_message(&self, session_id: &str, message_id: &str) -> anyhow::Result<()> {
        // 拿锁
        let conn = self.conn.lock().await;
        // 按 id 删；带上 session_id 是为了防止误删别的会话的消息
        conn.execute(
            "DELETE FROM messages WHERE session_id = ?1 AND id = ?2",
            params![session_id, message_id],
        )?;
        // 返回成功
        Ok(())
    }

    // 清空某个会话的消息，但保留会话本身
    pub async fn clear_messages(&self, session_id: &str) -> anyhow::Result<()> {
        // 拿锁
        let conn = self.conn.lock().await;
        // 删消息行
        conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        // 返回成功
        Ok(())
    }
}

// 让 Storage 可以被打印调试
impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 不打印连接本身，只说明它存在
        write!(f, "Storage {{ .. }}")
    }
}

// 编译期检查：Storage 必须同时是 Send 和 Sync，Tauri 的托管状态才有要求
#[allow(dead_code)]
fn _assert_send_sync<T: Send + Sync>() {}
const _: fn() = || _assert_send_sync::<Storage>();

// ============================================================================
// 单元测试：用内存数据库测，不需要真实文件
// 运行：cd src-tauri && cargo test
// ============================================================================
#[cfg(test)]
mod tests {
    // 引入被测的类型
    use super::*;

    // 造一条测试用的会话元信息
    fn meta(id: &str) -> SessionMeta {
        // 固定时间戳，方便断言
        SessionMeta {
            // id
            id: id.to_string(),
            // 标题
            title: "测试会话".to_string(),
            // 创建时间
            created_at: 100,
            // 更新时间
            updated_at: 100,
        }
    }

    // 造一条测试用的消息
    fn message(id: &str, role: &str, content: &str) -> StoredMessage {
        // 组装
        StoredMessage {
            // id
            id: id.to_string(),
            // 角色
            role: role.to_string(),
            // 正文
            content: content.to_string(),
            // 没有工具名
            tool_name: None,
            // 没有工具调用
            tool_call: None,
            // 创建时间
            created_at: 200,
        }
    }

    // 会话写进去能读出来
    #[tokio::test]
    async fn saves_and_lists_sessions() {
        // 建一个内存库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 存两个会话
        db.upsert_session(&meta("s1")).await.expect("应该能存");
        db.upsert_session(&meta("s2")).await.expect("应该能存");
        // 列出来
        let list = db.list_sessions().await.expect("应该能列出来");
        // 应该是两个
        assert_eq!(list.len(), 2);
        // 标题应该被正确保存
        assert!(list.iter().all(|s| s.title == "测试会话"));
    }

    // 更新会话标题不能把已经写进去的消息级联删掉。
    // INSERT OR REPLACE 会先删掉旧行；外键 ON DELETE CASCADE 会把消息一起删掉。
    #[tokio::test]
    async fn upsert_session_keeps_messages() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 先有会话和一条消息
        db.upsert_session(&meta("s1")).await.expect("应该能存会话");
        db.append_message("s1", 0, &message("m0", "user", "你好"))
            .await
            .expect("应该能写消息");
        // 只改标题，再存一次会话
        let mut renamed = meta("s1");
        renamed.title = "改过的标题".to_string();
        renamed.updated_at = 300;
        db.upsert_session(&renamed).await.expect("应该能更新会话");
        // 消息还在
        let messages = db.load_messages("s1").await.expect("应该能读消息");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "m0");
        assert_eq!(messages[0].content, "你好");
        // 标题是新的
        let list = db.list_sessions().await.expect("应该能列出会话");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "改过的标题");
        assert_eq!(list[0].updated_at, 300);
    }

    // 同一个 id 重复存是覆盖而不是新增
    #[tokio::test]
    async fn upsert_replaces_by_id() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 先存一次
        db.upsert_session(&meta("s1")).await.expect("应该能存");
        // 改标题再存一次
        let mut second = meta("s1");
        // 换标题
        second.title = "改过的标题".to_string();
        // 覆盖
        db.upsert_session(&second).await.expect("应该能覆盖");
        // 列出来
        let list = db.list_sessions().await.expect("应该能列出来");
        // 仍然只有一个
        assert_eq!(list.len(), 1);
        // 标题是新的
        assert_eq!(list[0].title, "改过的标题");
    }

    // 消息按 seq 顺序读出来
    #[tokio::test]
    async fn loads_messages_in_order() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 先建会话，否则外键约束会拒绝写入消息
        db.upsert_session(&meta("s1")).await.expect("应该能存会话");
        // 写三条消息，seq 分别是 2、0、1，故意乱序写入
        db.append_message("s1", 2, &message("m2", "assistant", "第三条"))
            .await
            .expect("应该能写");
        db.append_message("s1", 0, &message("m0", "user", "第一条"))
            .await
            .expect("应该能写");
        db.append_message("s1", 1, &message("m1", "assistant", "第二条"))
            .await
            .expect("应该能写");
        // 读出来
        let list = db.load_messages("s1").await.expect("应该能读出来");
        // 三条
        assert_eq!(list.len(), 3);
        // 顺序按 seq 排：m0、m1、m2
        assert_eq!(list[0].id, "m0");
        assert_eq!(list[1].id, "m1");
        assert_eq!(list[2].id, "m2");
    }

    // 删会话时消息跟着一起删（外键级联）
    #[tokio::test]
    async fn deletes_session_with_messages() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 建会话
        db.upsert_session(&meta("s1")).await.expect("应该能存会话");
        // 写一条消息
        db.append_message("s1", 0, &message("m0", "user", "你好"))
            .await
            .expect("应该能写");
        // 删会话
        db.delete_session("s1").await.expect("应该能删");
        // 消息应该也没了
        let messages = db.load_messages("s1").await.expect("应该能读");
        // 空列表
        assert!(messages.is_empty());
        // 会话也没了
        let list = db.list_sessions().await.expect("应该能列");
        // 空列表
        assert!(list.is_empty());
    }

    // 删单条消息
    #[tokio::test]
    async fn deletes_single_message() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 建会话
        db.upsert_session(&meta("s1")).await.expect("应该能存会话");
        // 写两条
        db.append_message("s1", 0, &message("m0", "user", "第一条"))
            .await
            .expect("应该能写");
        db.append_message("s1", 1, &message("m1", "assistant", "第二条"))
            .await
            .expect("应该能写");
        // 删第二条
        db.delete_message("s1", "m1").await.expect("应该能删");
        // 读出来
        let list = db.load_messages("s1").await.expect("应该能读");
        // 只剩一条
        assert_eq!(list.len(), 1);
        // 是第一条
        assert_eq!(list[0].id, "m0");
    }

    // 清空消息但保留会话
    #[tokio::test]
    async fn clears_messages_but_keeps_session() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 建会话
        db.upsert_session(&meta("s1")).await.expect("应该能存会话");
        // 写一条
        db.append_message("s1", 0, &message("m0", "user", "你好"))
            .await
            .expect("应该能写");
        // 清空消息
        db.clear_messages("s1").await.expect("应该能清空");
        // 消息空了
        assert!(db.load_messages("s1").await.expect("应该能读").is_empty());
        // 会话还在
        assert_eq!(db.list_sessions().await.expect("应该能列").len(), 1);
    }

    // 工具调用能原样存进去再读出来
    #[tokio::test]
    async fn round_trips_tool_call() {
        // 建库
        let db = Storage::open_in_memory().await.expect("应该能打开内存库");
        // 建会话
        db.upsert_session(&meta("s1")).await.expect("应该能存会话");
        // 造一条带工具调用的消息
        let mut msg = message("m0", "tool", "");
        // 工具名
        msg.tool_name = Some("echo".to_string());
        // 工具调用（JSON）
        msg.tool_call = Some(serde_json::json!({
            "id": "call_1",
            "type": "function",
            "function": { "name": "echo", "arguments": "{\"text\":\"hi\"}" }
        }));
        // 写进去
        db.append_message("s1", 0, &msg).await.expect("应该能写");
        // 读出来
        let list = db.load_messages("s1").await.expect("应该能读");
        // 工具名还在
        assert_eq!(list[0].tool_name.as_deref(), Some("echo"));
        // 工具调用也还在
        let call = list[0].tool_call.as_ref().expect("工具调用应该还在");
        // 函数名对得上
        assert_eq!(call["function"]["name"], "echo");
    }
}
