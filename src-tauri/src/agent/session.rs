// 引入消息、响应以及工具调用类型
use crate::protocol::{ChatMessage, ChatResponse, ToolCall};

// 把模型这一轮的回复（含工具调用）追加进历史，第二轮必须带上
pub fn append_assistant(messages: &mut Vec<ChatMessage>, result: &ChatResponse) {
    messages.push(ChatMessage {
        // 角色是助手
        role: "assistant".to_string(),
        // 这一轮的文字内容，通常为空
        content: result.content.clone(),
        // 带上模型发起的工具调用列表
        tool_calls: Some(result.tool_calls.clone()),
        // 助手消息不需要 tool_call_id
        tool_call_id: None,
    });
}

// 把工具结果作为一条 role=tool 的消息追加进历史
pub fn append_tool_result(messages: &mut Vec<ChatMessage>, call: &ToolCall, output: String) {
    messages.push(ChatMessage {
        // 角色是工具
        role: "tool".to_string(),
        // 工具的执行结果文本
        content: output,
        // 工具结果消息不带工具调用列表
        tool_calls: None,
        // 用 id 告诉模型这份结果对应哪一次调用
        tool_call_id: Some(call.id.clone()),
    });
}
