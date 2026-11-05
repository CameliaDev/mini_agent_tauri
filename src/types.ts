// 与 src-tauri/src/protocol.rs 对齐的数据结构，字段名就是 JSON 里的键名

// 模型发起的一次工具调用
export interface ToolCall {
  // 调用编号，后端回传工具结果时要对上
  id: string;
  // Rust 里叫 kind，序列化时 rename 成 "type"
  type: string;
  function: {
    // 工具名
    name: string;
    // JSON 格式的字符串参数
    arguments: string;
  };
}

// 发给模型的一条对话消息
export interface ChatMessage {
  // system / user / assistant / tool
  role: string;
  content: string;
  // 可选字段为 undefined 时 JSON.stringify 会自动省掉
  tool_calls?: ToolCall[];
  tool_call_id?: string;
}

// invoke("chat") 的请求体；tools 后端会覆盖，前端不用传
export interface ChatRequest {
  model: string;
  messages: ChatMessage[];
  temperature?: number;
}

// 一轮模型调用的汇总结果
export interface ChatResponse {
  content: string;
  usage_tokens: number | null;
  tool_calls: ToolCall[];
  finish_reason: string | null;
}

// StreamEvent 用 serde 默认的外部标签枚举表示
// TextDelta -> { "TextDelta": "..." }
// ToolCalls -> { "ToolCalls": [...] }
// ToolResult -> { "ToolResult": { call_id, name, output, is_error } }
// Cancelled -> "Cancelled"
// Done      -> "Done"
export interface ToolResultEvent {
  call_id: string;
  name: string;
  output: string;
  is_error: boolean;
}

export type StreamEvent =
  | { TextDelta: string }
  | { ToolCalls: ToolCall[] }
  | { ToolResult: ToolResultEvent }
  | "Cancelled"
  | "Done";

// 会话元信息，对应数据库 sessions 表的一行
export interface SessionMeta {
  id: string;
  title: string;
  // 毫秒时间戳
  created_at: number;
  updated_at: number;
}

// 落库的一条消息，对应数据库 messages 表的一行
export interface StoredMessage {
  id: string;
  role: string;
  content: string;
  tool_name?: string | null;
  tool_call?: ToolCall | null;
  created_at: number;
}
