// ============================================================================
// 全前端唯一认识 Tauri 流式接口的文件：invoke + Channel，把事件翻译成界面消息
//
// 为什么要单独一个文件：组件里一旦出现 invoke / Channel，就和后端绑死了。
// 把它们关在这里，组件就只跟 store 打交道，换后端或换协议时只改这一个文件。
//
// 本文件负责四件事：
//   1. 发送消息（sendMessage）
//   2. 停止生成（stopGeneration）—— 让后端打断正在进行的网络读取
//   3. 重新生成（regenerate）—— 删掉旧回复，用同样的历史再问一次
//   4. 每轮结束后把消息落库（persist）
// ============================================================================

// 引入 Tauri 的 invoke（调用后端命令）和 Channel（后端往前端推流式事件）
import { Channel, invoke } from "@tauri-apps/api/core";
// 引入前后端共用的数据结构
import type {
  ChatMessage,
  ChatRequest,
  ChatResponse,
  StreamEvent,
} from "../types";
// 引入界面消息类型和状态仓库
import type { UiMessage } from "../store";
import { activeMessages, useChatStore } from "../store";
// 引入持久化层的两个函数
import { persistSession, truncateAfter } from "./sessions";

// 后端固定用 DeepSeek，这里给出默认模型名
const DEFAULT_MODEL = "deepseek-chat";

// 把后端甩过来的 anyhow 字符串收成短提示。认不出来就原样显示
export function friendlyError(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  if (raw.includes("缺少 API Key") || raw.includes("DEEPSEEK_API_KEY")) {
    return "还没有配置 API Key。在项目根目录的 .env 里写上 DEEPSEEK_API_KEY，然后重启应用。";
  }
  if (/401|Unauthorized|invalid api key/i.test(raw)) {
    return "API Key 被拒绝了。检查 .env 里的 DEEPSEEK_API_KEY 是否正确。";
  }
  if (
    /error sending request|timed out|timeout|connection|dns|network|ECONN|os error/i.test(
      raw,
    )
  ) {
    return "网络连不上模型服务。检查网络后再试。";
  }
  return raw;
}

// 把界面消息转成发给后端的对话历史：只保留有内容的 user / assistant 文本消息
// 工具卡片不参与，因为后端那一轮的 tool 消息需要 tool_call_id 配对
function toHistory(messages: UiMessage[]): ChatMessage[] {
  return messages
    .filter(
      (m) =>
        (m.role === "user" || m.role === "assistant") && m.content.length > 0,
    )
    .map((m) => ({ role: m.role, content: m.content }));
}

// 跑一轮 agent：建通道、发请求、把事件翻译成 store 动作
// 发送消息和重新生成都要走这里，所以抽成公共函数，避免两份逻辑走偏
async function runAgent(history: ChatMessage[], sessionId: string) {
  // 停止点在气泡建好之前：用户消息留着，这一轮不再发请求
  if (useChatStore.getState().stopRequested) {
    useChatStore.setState({ isStreaming: false, stopRequested: false });
    await persistSession(sessionId);
    return;
  }
  // 建一条空助手消息，后续每个文字片段都往它身上追加
  const assistantId = useChatStore.getState().startAssistant();

  try {
    await runAgentChannel(history, assistantId);
  } catch (error) {
    // 网络错误、401、缺密钥、轮数超限都会走这里
    useChatStore.getState().fail(friendlyError(error), assistantId);
  } finally {
    // 无论成功失败都落库，保证界面上能看到的对话都存下来了
    await persistSession(sessionId);
  }
}

// 建通道并发起请求。Channel 构造失败也要能落到上面的 catch
async function runAgentChannel(history: ChatMessage[], assistantId: string) {
  // 建 Channel，把后端事件翻译成界面消息
  // 必须在 invoke 之前设好回调，否则早期事件会丢失
  const onEvent = new Channel<StreamEvent>((event) => {
    // 每次回调都重新取一次状态，保证拿到的是最新值
    const s = useChatStore.getState();
    // 单元变体 Done 序列化后就是一个字符串
    if (event === "Done") {
      // 后端说完了
      s.finish(assistantId);
      return;
    }
    // 单元变体 Cancelled：保留半截文字，并标成已停止
    if (event === "Cancelled") {
      s.markStopped(assistantId);
      return;
    }
    // 文字片段：追加到当前那条助手消息末尾
    if ("TextDelta" in event) {
      s.appendDelta(assistantId, event.TextDelta);
      return;
    }
    // 工具调用：插一张运行中的卡片
    if ("ToolCalls" in event) {
      s.addToolCards(event.ToolCalls, assistantId);
      return;
    }
    // 工具跑完：写上输出，卡片默认折叠
    if ("ToolResult" in event) {
      s.completeTool(
        event.ToolResult.call_id,
        event.ToolResult.output,
        event.ToolResult.is_error,
      );
    }
  });

  // 组装请求：模型名 + 对话历史
  // tools 不用传，后端会用自己的工具清单覆盖
  const req: ChatRequest = {
    model: DEFAULT_MODEL,
    messages: history,
  };

  // Tauri 2 命令参数默认 camelCase：Rust 的 on_event 对应这里的 onEvent
  // state 是框架注入的，前端不用传
  await invoke<ChatResponse>("chat", { req, onEvent });
  // invoke resolve 表示这一趟结束，兜底收尾（后端只在部分路径发 Done）
  // 如果已经标成停止或失败，finish 不会把标记清掉
  useChatStore.getState().finish(assistantId);
}

// 发送一条用户消息，跑完整 agent 流程
export async function sendMessage(text: string): Promise<void> {
  // 拿到当前状态快照（zustand 的 action 是稳定的，可以直接调用）
  const store = useChatStore.getState();
  // 正在生成时不重复发起
  if (store.isStreaming) return;
  // 当前会话 id：落库时要用
  const sessionId = store.activeSessionId;
  // 先换成停止按钮，再写入消息。发送的这一下就能停
  store.beginTurn();

  // 1) 先把用户消息放进当前会话，再取历史——这样新消息也在历史里
  store.appendUser(text);
  const history = toHistory(activeMessages(useChatStore.getState()));

  // 2) 跑这一轮 agent
  await runAgent(history, sessionId);
}

// 停止生成：让后端打断正在进行的网络读取
export async function stopGeneration(): Promise<void> {
  // 请求可能还没进后端（重新生成要先删数据库）。先在本地记一笔，别等令牌
  useChatStore.getState().noteStop();
  // 调后端的 stop_generation 命令，它会把取消令牌置为 true
  await invoke("stop_generation");
}

// 重新生成：删掉最后一条助手回复，用同样的历史再问一次
export async function regenerate(): Promise<void> {
  // 拿到当前状态
  const store = useChatStore.getState();
  // 正在生成时不重复发起
  if (store.isStreaming) return;
  // 当前会话的消息
  const messages = activeMessages(store);
  // 找最后一条助手消息
  const lastAssistant = [...messages]
    .reverse()
    .find((m) => m.role === "assistant");
  // 没有助手回复就没什么可重新生成的
  if (!lastAssistant) return;
  // 它前面那条用户消息就是这次的提问
  const index = messages.findIndex((m) => m.id === lastAssistant.id);
  const lastUser = [...messages.slice(0, index)]
    .reverse()
    .find((m) => m.role === "user");
  // 找不到用户消息就放弃
  if (!lastUser) return;
  // 当前会话 id
  const sessionId = store.activeSessionId;
  // 确认能重新生成之后再换成停止。后面还有一次数据库删除
  store.beginTurn();

  // 1) 内存里先截断：删掉那条助手回复以及它之后的所有内容
  store.truncateAfter(sessionId, lastUser.id);
  // 2) 数据库里也截断，两边保持一致。失败就把停止按钮收回来
  try {
    await truncateAfter(sessionId, lastUser.id);
  } catch (error) {
    useChatStore.setState({ isStreaming: false, error: friendlyError(error) });
    return;
  }
  // 3) 直接跑 agent，不走 sendMessage
  //    关键：那条用户消息还在历史里，不能再 appendUser 一次，否则会多出一条重复的提问
  const history = toHistory(activeMessages(useChatStore.getState()));
  await runAgent(history, sessionId);
}
