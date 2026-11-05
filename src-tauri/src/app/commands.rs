// ============================================================================
// 这一层是「后端 ↔ 前端」的翻译层：
//   前端来的：一次 invoke 调用（请求 + 事件通道）
//   后端去的：一次 agent 循环（agent::run_agent）
//   回前端去的：流式事件（Channel）+ 最终结果（返回值）
//
// 关键设计：agent 层只认内部的 tokio 通道，不认 Tauri 的 Channel，
// 这样 agent 那一层可以脱离 Tauri 单独读懂、单独测试
// ============================================================================

// 引入 Tauri 的 IPC 通道，用来把流式事件推给前端
use tauri::ipc::Channel;
// 引入 agent 主循环
use crate::agent::run_agent;
// 引入流式事件枚举
use crate::agent::events::StreamEvent;
// 引入应用共享状态
use crate::app::state::AppState;
// 引入聊天请求、响应数据结构
use crate::protocol::{ChatRequest, ChatResponse, SessionMeta, StoredMessage};
// 引入存储层
use crate::storage::Storage;

// 模板自带的示例命令，保留不动
#[tauri::command]
pub(crate) fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// 定义聊天命令：前端传请求和事件通道，后端跑完整的 agent 循环
#[tauri::command]
pub(crate) async fn chat(
    // 从 Tauri 状态里取出模型配置（框架注入，前端不用传）
    state: tauri::State<'_, AppState>,
    // 前端传入的通道，后端通过它逐条推送流式事件
    on_event: Channel<StreamEvent>,
    // 前端传来的聊天请求
    req: ChatRequest,
) -> Result<ChatResponse, String> {
    // 0) 准备取消信号：用户点「停止生成」时，stop_generation 命令会触发它
    //    顺序很重要：先 reset 清掉上一轮的取消标记，再 subscribe 订阅。
    //    反过来的话，订阅者会先看到「已取消」的旧值，导致这一轮刚起步就被打断。
    state.cancel.reset();
    let cancel = state.cancel.subscribe();
    // 1) 建一条「内部通道」：agent 层往 tx 推事件，前端的 Channel 由下面的转发任务喂
    // 容量 64 是缓冲区大小，满了发送方会等待，不会丢事件
    let (tx, mut rx) = tauri::async_runtime::channel::<StreamEvent>(64);
    // 2) 起一个转发任务，把内部通道的事件原样搬到前端通道
    let forward = tokio::spawn(async move {
        // 一直收到所有 tx 被释放为止
        while let Some(event) = rx.recv().await {
            // 转发失败（前端关了页面）就忽略，不影响主流程
            let _ = on_event.send(event);
        }
    });
    // 3) 真正的活交给 agent 层：它把文字和工具调用一条条推进 tx
    let result = run_agent(state.config.clone(), tx.clone(), req, cancel).await;
    // 4) 收尾：释放 command 手里的发送端
    // 只有所有 Sender 都被释放，rx 才会收到 None，转发任务也才会结束
    drop(tx);
    // 5) 等转发任务把剩余事件都推完再返回，避免前端漏掉最后几个字
    let _ = forward.await;
    // 6) 把 anyhow 错误转成字符串交给前端（前端 invoke 会 reject）
    result.map_err(|e| e.to_string())
}

// ============================================================================
// 停止生成：打断当前正在跑的那一轮
// ============================================================================

// 前端点「停止生成」时调用这个命令
// 它不直接杀线程，而是把取消令牌置为 true；
// 正在读网络流的地方（providers/sse.rs）会收到信号并优雅退出
#[tauri::command]
pub(crate) fn stop_generation(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // 触发取消：所有订阅者都会收到
    state.cancel.cancel();
    // 没有错误
    Ok(())
}

// ============================================================================
// 会话持久化：下面这组命令是 SQLite 存储层对外的唯一入口
// ============================================================================

// 列出全部会话（侧栏用），按更新时间倒序
#[tauri::command]
pub(crate) async fn list_sessions(
    storage: tauri::State<'_, Storage>,
) -> Result<Vec<SessionMeta>, String> {
    // 数据库操作失败就把错误转成字符串给前端
    storage.list_sessions().await.map_err(|e| e.to_string())
}

// 读出某个会话的全部消息（前端启动或切换会话时调用）
#[tauri::command]
pub(crate) async fn load_messages(
    storage: tauri::State<'_, Storage>,
    session_id: String,
) -> Result<Vec<StoredMessage>, String> {
    // 按会话 id 查消息
    storage.load_messages(&session_id).await.map_err(|e| e.to_string())
}

// 保存（新建或覆盖）一个会话的元信息
#[tauri::command]
pub(crate) async fn save_session(
    storage: tauri::State<'_, Storage>,
    session: SessionMeta,
) -> Result<(), String> {
    // upsert：id 存在就替换，不存在就插入
    storage.upsert_session(&session).await.map_err(|e| e.to_string())
}

// 追加一条消息到某个会话
// seq 是这条消息在会话里的序号（从 0 开始），决定显示顺序
#[tauri::command]
pub(crate) async fn append_message(
    storage: tauri::State<'_, Storage>,
    session_id: String,
    seq: i64,
    message: StoredMessage,
) -> Result<(), String> {
    // 写入一条
    storage
        .append_message(&session_id, seq, &message)
        .await
        .map_err(|e| e.to_string())
}

// 删除一个会话（消息随外键级联一起删）
#[tauri::command]
pub(crate) async fn delete_session(
    storage: tauri::State<'_, Storage>,
    session_id: String,
) -> Result<(), String> {
    // 删除会话
    storage.delete_session(&session_id).await.map_err(|e| e.to_string())
}

// 删除某个会话里从某条消息开始的所有消息（重新生成时用）
// after_id 是「保留的最后一条」，它之后的都被删掉
#[tauri::command]
pub(crate) async fn delete_session_messages(
    storage: tauri::State<'_, Storage>,
    session_id: String,
    after_id: String,
) -> Result<(), String> {
    // 先查出那条消息的 seq
    let messages = storage
        .load_messages(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    // 找到它的位置；找不到就什么都不做（不报错，因为这是幂等操作）
    let Some(index) = messages.iter().position(|m| m.id == after_id) else {
        // 直接返回成功
        return Ok(());
    };
    // 把它之后的消息逐条删掉
    for message in messages.iter().skip(index + 1) {
        // 复用 append_message 的删除能力：这里直接调存储层
        storage
            .delete_message(&session_id, &message.id)
            .await
            .map_err(|e| e.to_string())?;
    }
    // 全部删完
    Ok(())
}

// 清空某个会话的全部消息，但保留会话本身
#[tauri::command]
pub(crate) async fn clear_session_messages(
    storage: tauri::State<'_, Storage>,
    session_id: String,
) -> Result<(), String> {
    // 清空
    storage
        .clear_messages(&session_id)
        .await
        .map_err(|e| e.to_string())
}
