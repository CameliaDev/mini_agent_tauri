// ============================================================================
// 会话持久化：前端和 SQLite 之间的桥
//
// 这一层负责三件事：
//   1. 启动时把数据库里的会话读回来（hydrate）
//   2. 每轮对话结束后把这一轮的消息写回去（persist）
//   3. 切换 / 删除 / 清空会话时同步数据库
//
// 为什么单独一个文件：ipc/chat.ts 负责「流式聊天」，ipc/sessions.ts 负责「落库」。
// 两个文件都只碰后端命令，组件永远不直接 invoke。
// ============================================================================

// 引入 Tauri 的 invoke
import { invoke } from "@tauri-apps/api/core";
// 引入前后端共用的数据结构
import type { SessionMeta, StoredMessage } from "../types";
// 引入界面消息类型和状态仓库
import type { UiMessage } from "../store";
import { useChatStore } from "../store";

// 把界面消息转成能落库的形状
function toStored(message: UiMessage): StoredMessage {
  return {
    // 消息 id 沿用界面上的 id，这样重新落库时是覆盖而不是新增
    id: message.id,
    // 角色
    role: message.role,
    // 正文
    content: message.content,
    // 工具名（只有工具消息有）
    tool_name: message.toolName ?? null,
    // 完整工具调用（只有工具消息有）
    tool_call: message.toolCall ?? null,
    // 落库时间：优先用消息自己的创建时间，没有就退回当前时间
    created_at: message.createdAt ?? Date.now(),
  };
}

// 把数据库里的消息转回界面消息
function toUi(message: StoredMessage): UiMessage {
  return {
    // id
    id: message.id,
    // 角色：数据库里是字符串，这里收窄成界面用的联合类型
    role: message.role as UiMessage["role"],
    // 正文
    content: message.content,
    // 工具名
    toolName: message.tool_name ?? undefined,
    // 工具调用
    toolCall: message.tool_call ?? undefined,
    // 创建时间
    createdAt: message.created_at,
  };
}

// 启动时读回全部会话，并把当前会话的消息也读出来
export async function hydrateFromDb(): Promise<void> {
  // 读会话列表
  const metas = await invoke<SessionMeta[]>("list_sessions");
  // 塞进 store（只带元信息，消息按需再读）
  useChatStore.getState().hydrateSessions(metas);
  // 数据库里一个会话都没有：把内存里那个空会话落库，界面上就有东西了
  if (metas.length === 0) {
    // 拿到当前状态
    const state = useChatStore.getState();
    // 找到当前会话
    const current = state.sessions.find((s) => s.id === state.activeSessionId);
    // 有就落库
    if (current) await persistSession(current.id);
    // 不用读消息，本来就是空的
    return;
  }
  // 把当前会话的消息读回来
  await loadMessagesOf(useChatStore.getState().activeSessionId);
}

// 读某个会话的全部消息，填进 store
export async function loadMessagesOf(sessionId: string): Promise<void> {
  // 从数据库读
  const messages = await invoke<StoredMessage[]>("load_messages", {
    // Tauri 2 参数默认 camelCase，Rust 侧是 session_id
    sessionId,
  });
  // 转成界面消息后填进 store
  useChatStore.getState().setSessionMessages(
    sessionId,
    messages.map(toUi),
  );
}

// 把某个会话的元信息 + 全部消息写回数据库
export async function persistSession(sessionId: string): Promise<void> {
  // 拿当前状态
  const state = useChatStore.getState();
  // 找到目标会话
  const session = state.sessions.find((s) => s.id === sessionId);
  // 会话不存在就不用写
  if (!session) return;
  // 先写会话元信息
  await invoke("save_session", {
    // Rust 侧参数名是 session
    session: {
      // 会话 id
      id: session.id,
      // 标题
      title: session.title,
      // 创建时间
      created_at: session.createdAt,
      // 更新时间：每次落库都刷新，侧栏排序靠它
      updated_at: Date.now(),
    },
  });
  // 再逐条写消息；seq 用下标，正好就是对话顺序
  for (const [index, message] of session.messages.entries()) {
    // 写入一条
    await invoke("append_message", {
      // 所属会话
      sessionId: session.id,
      // 顺序号
      seq: index,
      // 消息本体
      message: toStored(message),
    });
  }
}

// 删除一个会话（数据库那边的消息会随外键级联一起删）
export async function removeSession(sessionId: string): Promise<void> {
  // 调后端命令
  await invoke("delete_session", {
    // Rust 侧参数名是 session_id
    sessionId,
  });
}

// 清空某个会话的消息，但保留会话本身
export async function clearMessages(sessionId: string): Promise<void> {
  // 调后端命令
  await invoke("clear_session_messages", {
    // Rust 侧参数名是 session_id
    sessionId,
  });
}

// 删掉某条消息之后的所有消息（重新生成时把旧回复抹掉）
export async function truncateAfter(sessionId: string, messageId: string): Promise<void> {
  // 调后端命令
  await invoke("delete_session_messages", {
    // Rust 侧参数名是 session_id
    sessionId,
    // Rust 侧参数名是 after_id
    afterId: messageId,
  });
}
