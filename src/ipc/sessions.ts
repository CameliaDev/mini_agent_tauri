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
    // 库里没有状态列。有正文且以失败前缀开头算失败，否则算已经结束
    toolStatus:
      message.role === "tool"
        ? message.content.startsWith("工具执行失败")
          ? "error"
          : "done"
        : undefined,
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

// 读某个会话的全部消息，填进 store。
// 内存已经接管的会话不再读库：切走期间库里的快照往往更旧，读回来会把消息盖没。
export async function loadMessagesOf(sessionId: string): Promise<void> {
  // 已经有权威副本就直接用内存
  const current = useChatStore.getState().sessions.find((s) => s.id === sessionId);
  if (!current || current.messagesLoaded) return;
  // 读库期间用户可能已经发出新消息。那种会话灌完要按完整列表再落一次库，序号才不会冲突
  const hadLocal = current.messages.length > 0;
  // 从数据库读
  const messages = await invoke<StoredMessage[]>("load_messages", {
    // Tauri 2 参数默认 camelCase，Rust 侧是 session_id
    sessionId,
  });
  // 灌入前再看一次：等待期间可能已经清空，或另一笔 load 先灌完了
  const latest = useChatStore.getState().sessions.find((s) => s.id === sessionId);
  if (!latest || latest.messagesLoaded) return;
  // set 回调里还会看一次 messagesLoaded，避免覆盖加载期间新增的消息
  useChatStore.getState().setSessionMessages(
    sessionId,
    messages.map(toUi),
  );
  if (hadLocal) await persistSession(sessionId);
}

// 同一个会话的落库必须排队。
// 切走时会先写一版，这一轮结束时再写一版；若两版并行，慢的那版可能用更短的正文把后写的完整正文盖掉。
const persistChain = new Map<string, Promise<void>>();

// 把某个会话的元信息 + 全部消息写回数据库
export function persistSession(sessionId: string): Promise<void> {
  const prev = persistChain.get(sessionId) ?? Promise.resolve();
  // 上一笔失败也不挡住下一笔，下一笔会按当时的内存重写
  const next = prev.catch(() => undefined).then(() => writeSession(sessionId));
  const tracked = next.finally(() => {
    if (persistChain.get(sessionId) === tracked) persistChain.delete(sessionId);
  });
  persistChain.set(sessionId, tracked);
  return tracked;
}

// 真正写入。调用时再读内存，这样排在后面的那笔能看到流式输出长出来的文字。
async function writeSession(sessionId: string): Promise<void> {
  const session = useChatStore.getState().sessions.find((s) => s.id === sessionId);
  // 还没从库里灌进来时，内存里的 messages 不是全量。
  // 按残缺列表重写 seq，会和库里已有的消息抢同一个序号。
  if (!session || !session.messagesLoaded) return;
  // 先记下这一笔要写的消息。写入过程中流式输出还会变，下一笔排队的落库会带上新文字。
  const messages = session.messages;
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
      // 用会话自己的更新时间。切走时落库不应把「刚看过」写成「刚更新」，
      // 否则重启后按 updated_at 排序，旧会话会冒到新建对话前面。
      updated_at: session.updatedAt,
    },
  });
  // 再逐条写消息；seq 用下标，正好就是对话顺序
  for (const [index, message] of messages.entries()) {
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
