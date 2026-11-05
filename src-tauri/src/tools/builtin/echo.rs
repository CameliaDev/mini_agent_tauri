// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入工具统一接口
use crate::tools::Tool;

// 定义内置工具二：把文本原样返回，用来验证带参数的调用
pub struct Echo;

// 为它实现 Tool 接口
#[async_trait]
impl Tool for Echo {
    // 工具名
    fn name(&self) -> &'static str {
        // 与注册表里插入的键保持一致
        "echo"
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
                description: "把传入的文本原样返回，用于测试带参数的工具调用".to_string(),
                // 声明一个必填的字符串参数 text
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "要原样返回的文本"
                        }
                    },
                    "required": ["text"]
                }),
            },
        }
    }

    // 执行：从参数里取 text 字段
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        // 取不到就给个明确提示，模型会看到这个结果
        let text = args["text"]
            .as_str()
            .unwrap_or("(没有收到 text 参数)")
            .to_string();
        // 包装成成功结果返回
        Ok(text)
    }
}
