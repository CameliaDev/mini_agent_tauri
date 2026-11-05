// 引入 async_trait 宏，让 trait 里可以写 async fn
use async_trait::async_trait;
// 引入 HashMap，用来按工具名存放工具实例
use std::collections::HashMap;
// 引入工具调用和工具描述相关的数据结构
use crate::protocol::{ToolCall, ToolDefinition};

// 声明内置工具子模块，Rust 会去加载 src/tools/builtin/mod.rs
pub mod builtin;

// 定义所有工具都要实现的统一接口
#[async_trait]
pub trait Tool: Send + Sync {
    // 返回工具的唯一名字，模型靠它点名
    fn name(&self) -> &'static str;
    // 返回工具的说明书，会随请求发给模型
    fn definition(&self) -> ToolDefinition;
    // 真正执行工具，参数是模型生成的 JSON，返回给模型的文本
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String>;
}

// 定义工具注册表，负责说明书收集与按名分发
pub struct ToolRegistry {
    // 工具名到工具实例的映射
    tools: HashMap<String, Box<dyn Tool>>,
}

// 为注册表实现方法
impl ToolRegistry {
    // 创建带内置工具的注册表
    pub fn builtin() -> Self {
        // 先建一个空表
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        // 注册「获取当前时间」工具
        tools.insert(
            "get_current_time".to_string(),
            Box::new(builtin::time::GetCurrentTime),
        );
        // 注册「回显文本」工具
        tools.insert("echo".to_string(), Box::new(builtin::echo::Echo));
        // 返回装好工具的注册表
        Self { tools }
    }

    // 生成工具清单，作为请求体里的 tools 字段
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        // 遍历所有工具，收集各自的说明书
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    // 按模型给的订单找到工具并执行
    pub async fn execute(&self, call: &ToolCall) -> anyhow::Result<String> {
        // 按名字查表，找不到就报错
        let tool = self
            .tools
            .get(&call.function.name)
            .ok_or_else(|| anyhow::anyhow!("未知工具: {}", call.function.name))?;
        // 参数是 JSON 字符串，解析成 Value；解析失败就当成空值
        let args: serde_json::Value =
            serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);
        // 调用工具自己的执行逻辑
        tool.execute(args).await
    }
}
