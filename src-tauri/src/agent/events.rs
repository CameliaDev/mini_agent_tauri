// 引入 Serialize，让流式事件能通过 Tauri 的 IPC 通道发给前端
use serde::Serialize;
// 引入订单类型，ToolCalls 变体里要装它
use crate::protocol::ToolCall;

// 一个工具跑完后的结果。前端靠 call_id 对上刚才那张卡片
#[derive(Debug, Clone, Serialize)]
pub struct ToolResultEvent {
    // 和 ToolCall.id 相同
    pub call_id: String,
    // 工具名，卡片上已经有了，这里再带一份，避免对不上时只能显示空白
    pub name: String,
    // 给模型看的那段输出，界面默认折叠
    pub output: String,
    // true 表示工具返回了错误文本（以「工具执行失败」开头）
    pub is_error: bool,
}

// 定义流式事件：让一条通道既能推文字，也能推工具调用
#[derive(Debug, Clone, Serialize)]
pub enum StreamEvent {
    // 一小段正文文字，对应原来的 Sender<String>
    TextDelta(String),
    // 一组完整的工具调用订单，流解析完后一次性给出
    ToolCalls(Vec<ToolCall>),
    // 某一个工具执行结束
    ToolResult(ToolResultEvent),
    // 用户点了「停止生成」：本轮提前结束，已收到的文字保留
    Cancelled,
    // 整个 agent 流程结束，上层可以收工
    Done,
}
