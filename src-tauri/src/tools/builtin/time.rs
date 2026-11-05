// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入工具统一接口
use crate::tools::Tool;

// 定义内置工具一：获取当前本地时间
pub struct GetCurrentTime;

// 为它实现 Tool 接口
#[async_trait]
impl Tool for GetCurrentTime {
    // 工具名
    fn name(&self) -> &'static str {
        // 与注册表里插入的键保持一致
        "get_current_time"
    }

    // 工具说明书
    fn definition(&self) -> ToolDefinition {
        // 构造说明书
        ToolDefinition {
            // 协议固定值
            kind: "function".to_string(),
            // 函数部分
            function: ToolFunction {
                // 工具名
                name: self.name().to_string(),
                // 告诉模型这个工具能干什么
                description: "获取当前的本地日期和时间，不需要任何参数".to_string(),
                // 无参数的 JSON Schema
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        }
    }

    // 执行：不需要参数，直接返回格式化后的当前时间
    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<String> {
        // 用 chrono 取本地时间并格式化
        Ok(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
    }
}
