// 引入 serde 的两个派生宏：Serialize 发给前端，Deserialize 接收前端参数
use serde::{Deserialize, Serialize};

// 聊天消息：补上工具调用相关的两个可选字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    // 角色：system / user / assistant / tool
    pub role: String,
    // 正文；assistant 只回工具调用时这里是空串
    pub content: String,
    // assistant 消息携带的订单列表；为 None 时整个字段不出现在 JSON 里
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    // role 为 tool 时必填，用来和上面 ToolCall 的 id 对上
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

// 一次聊天请求：追加工具清单
#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    // 完整对话历史
    pub messages: Vec<ChatMessage>,
    // 具体模型名
    pub model: String,
    // 温度，可不传
    pub temperature: Option<f32>,
    // 本次允许模型使用的工具；前端不传时自动当成空列表，不破坏现有调用
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}

// 一次聊天结果：追加订单和结束原因
#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    // 拼好的完整文字
    pub content: String,
    // token 数，暂不统计
    pub usage_tokens: Option<u32>,
    // 模型这次发起的工具调用；非空就说明要执行工具并发第二轮
    pub tool_calls: Vec<ToolCall>,
    // 结束原因："stop" 正常说完 / "tool_calls" 等你执行工具
    pub finish_reason: Option<String>,
}

// 定义「一个工具的说明书」，随请求发给模型让它知道有哪些能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    // 协议要求字段名叫 type，Rust 里 type 是关键字所以改叫 kind 再重命名
    #[serde(rename = "type")]
    // 固定填 "function"，OpenAI 兼容协议目前只有这一种
    pub kind: String,
    // 函数名、描述、参数结构都装在这个嵌套对象里
    pub function: ToolFunction,
}

// 定义工具说明书里的 function 部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    // 工具名，模型靠它点名，例如 "read_file"
    pub name: String,
    // 自然语言说明这个工具能干什么，写得清楚模型才选得准
    pub description: String,
    // 参数的 JSON Schema，用 Value 是为了能装任意结构
    pub parameters: serde_json::Value,
}

// 定义「模型发起的一次工具调用」，也就是那张订单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    // 本次调用的唯一编号，回传工具结果时必须原样带上
    pub id: String,
    // 同样因为 type 是关键字，序列化时改回 "type"
    #[serde(rename = "type")]
    // 固定填 "function"
    pub kind: String,
    // 里面是工具名和参数
    pub function: ToolCallFunction,
}

// 定义订单里的函数名与参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    // 模型选中的工具名
    pub name: String,
    // 参数：注意是「JSON 格式的字符串」，不是对象，要自己 parse
    pub arguments: String,
}

// ============================================================================
// 持久化相关的数据结构（前端 ↔ 后端 共用）
// ============================================================================

// 定义「一个会话的元信息」，对应数据库 sessions 表的一行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    // 会话 id，由前端生成（UUID 字符串），数据库主键
    pub id: String,
    // 会话标题，默认「新对话」，发出第一条消息后自动改成消息摘要
    pub title: String,
    // 创建时间（毫秒时间戳）
    pub created_at: i64,
    // 最后更新时间（毫秒时间戳），侧栏按它排序和分组
    pub updated_at: i64,
}

// 定义「一条落库的消息」，对应数据库 messages 表的一行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    // 消息 id，由前端生成，数据库主键
    pub id: String,
    // 角色：user / assistant / tool
    pub role: String,
    // 正文；工具消息这里是空串
    pub content: String,
    // 工具名，只有 role=tool 时有值
    #[serde(default)]
    pub tool_name: Option<String>,
    // 完整工具调用（JSON 对象），只有 role=tool 时有值
    #[serde(default)]
    pub tool_call: Option<serde_json::Value>,
    // 创建时间（毫秒时间戳）
    pub created_at: i64,
}
