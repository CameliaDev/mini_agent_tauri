// 引入 Serialize，让流式事件能通过 Tauri 的 IPC 通道发给前端
use serde::Serialize;
// 引入订单类型，ToolCalls 变体里要装它
use crate::protocol::ToolCall;

// 定义流式事件：让一条通道既能推文字，也能推工具调用
#[derive(Debug, Clone, Serialize)]
pub enum StreamEvent {
    // 一小段正文文字，对应原来的 Sender<String>
    TextDelta(String),
    // 一组完整的工具调用订单，流解析完后一次性给出
    ToolCalls(Vec<ToolCall>),
    // 用户点了「停止生成」：本轮提前结束，已收到的文字保留
    Cancelled,
    // 整个 agent 流程结束，上层可以收工
    Done,
}
