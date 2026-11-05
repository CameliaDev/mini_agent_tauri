// 引入 async_trait 宏，让 trait 里可以写 async fn
use async_trait::async_trait;
// 引入 HashMap，用来按工具名存放工具实例
use std::collections::HashMap;
// 引入工具调用和工具描述相关的数据结构
use crate::protocol::{ToolCall, ToolDefinition};
// bash 要在命令还在跑的时候看见「停止」，所以注册表把取消信号传进去
use crate::agent::cancel::CancelSignal;

// 声明内置工具子模块，Rust 会去加载 src/tools/builtin/mod.rs
pub mod builtin;
// 声明沙箱子模块：工作区根、路径逃逸检查、原子写入
pub mod sandbox;

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
    // 用环境变量（或当前目录）决定工作区，注册全部内置工具
    pub fn builtin() -> Self {
        Self::with_workspace(sandbox::load_workspace_root())
    }

    // 用调用方给的根目录注册工具。测试传临时目录。这条路径没有取消信号
    pub fn with_workspace(root: impl Into<std::path::PathBuf>) -> Self {
        Self::with_workspace_and_cancel(root, None)
    }

    // 应用走这里：根目录和这一轮的取消信号一起传进来，bash 才能在停止时杀进程
    pub fn with_workspace_and_cancel(
        root: impl Into<std::path::PathBuf>,
        cancel: Option<CancelSignal>,
    ) -> Self {
        let root = root.into();
        // 先建一个空表
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        // 这两个工具不碰文件，根目录对它们没有意义
        register(&mut tools, Box::new(builtin::time::GetCurrentTime));
        register(&mut tools, Box::new(builtin::echo::Echo));
        // 四个文件工具共用同一个根，整轮对话的沙箱边界才一致
        register(
            &mut tools,
            Box::new(builtin::read::ReadFile::new(root.clone())),
        );
        register(
            &mut tools,
            Box::new(builtin::write::WriteFile::new(root.clone())),
        );
        register(
            &mut tools,
            Box::new(builtin::edit::EditFile::new(root.clone())),
        );
        register(
            &mut tools,
            Box::new(builtin::list_dir::ListDir::new(root.clone())),
        );
        // bash 的当前目录也是这个根。取消信号只有它会用
        register(&mut tools, Box::new(builtin::bash::Bash::new(root, cancel)));
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

// 用工具自己报的名字当键，避免注册时手写的字符串和 name() 对不上
fn register(tools: &mut HashMap<String, Box<dyn Tool>>, tool: Box<dyn Tool>) {
    let name = tool.name().to_string();
    tools.insert(name, tool);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ToolCall, ToolCallFunction};
    use crate::tools::sandbox::TestWorkspace;
    use std::collections::HashSet;

    #[test]
    fn registers_builtin_tools() {
        let ws = TestWorkspace::new();
        let registry = ToolRegistry::with_workspace(ws.path());
        let names: HashSet<String> = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name)
            .collect();
        for name in [
            "get_current_time",
            "echo",
            "read",
            "write",
            "edit",
            "list_dir",
            "bash",
        ] {
            assert!(names.contains(name), "缺少工具 {name}，现有 {names:?}");
        }
        let read = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.function.name == "read")
            .expect("应该有 read");
        assert_eq!(read.kind, "function");
        let required = read.function.parameters["required"]
            .as_array()
            .expect("required 应该是数组");
        assert!(required.iter().any(|item| item == "path"));
        let bash = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.function.name == "bash")
            .expect("应该有 bash");
        let bash_required = bash.function.parameters["required"]
            .as_array()
            .expect("required 应该是数组");
        assert!(bash_required.iter().any(|item| item == "command"));
    }

    #[tokio::test]
    async fn executes_read_by_name() {
        let ws = TestWorkspace::new();
        ws.write("note.txt", "hello");
        let registry = ToolRegistry::with_workspace(ws.path());
        let call = ToolCall {
            id: "call_1".to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: "read".to_string(),
                arguments: r#"{"path":"note.txt"}"#.to_string(),
            },
        };
        let out = registry.execute(&call).await.expect("read 应该成功");
        assert_eq!(out, "hello");
    }
}
