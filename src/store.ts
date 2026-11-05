// 全应用唯一的状态存放处：组件只订阅这里，不认识 Tauri、不认识流式事件
import { create } from "zustand";
import type { SessionMeta, ToolCall } from "./types";

// 界面上的一条消息，比后端 ChatMessage 多了展示用的状态
export interface UiMessage {
  id: string;
  role: "user" | "assistant" | "tool" | "error";
  content: string;
  // role 为 tool 时显示的工具名
  toolName?: string;
  // role 为 tool 时保存完整的工具调用
  toolCall?: ToolCall;
  // 是否正在流式追加文字
  streaming?: boolean;
  // 创建时间（毫秒时间戳），落库时用
  createdAt?: number;
}

// 一个会话，对应左侧栏里的一项
export interface Session {
  id: string;
  title: string;
  messages: UiMessage[];
  createdAt: number;
  updatedAt: number;
}

interface ChatState {
  // 所有会话，顺序就是侧栏里的顺序（新的在前）
  sessions: Session[];
  // 当前正在看的会话
  activeSessionId: string;
  isStreaming: boolean;
  error: string | null;
  // 输入框草稿
  draft: string;
  setDraft: (text: string) => void;
  // 会话管理
  newSession: () => void;
  selectSession: (id: string) => void;
  deleteSession: (id: string) => void;
  clearActiveSession: () => void;
  // 用数据库里的会话列表替换内存中的列表（启动时调用）
  hydrateSessions: (metas: SessionMeta[]) => void;
  // 用数据库里的消息替换某个会话的消息（切换会话时调用）
  setSessionMessages: (sessionId: string, messages: UiMessage[]) => void;
  // 删掉某条消息之后的所有消息（重新生成前调用）
  truncateAfter: (sessionId: string, messageId: string) => void;
  // 消息操作
  // appendUser / startAssistant 作用在当前会话上；
  // 其余几个按「消息 id」定位所属会话，用户中途切走也不会写错地方
  appendUser: (text: string) => void;
  startAssistant: () => string;
  appendDelta: (id: string, text: string) => void;
  addToolCards: (calls: ToolCall[], anchorId: string) => void;
  finish: (anchorId?: string) => void;
  fail: (message: string, anchorId?: string) => void;
  clearError: () => void;
}

// 空数组常量：作为 selector 的兜底值必须是稳定引用，
// 否则每次渲染都是新数组，React 会认为状态一直在变（无限重渲染）
const EMPTY: UiMessage[] = [];

// 每条消息一个唯一 id，流式追加靠它定位「该往哪一条上写」
const newId = () => crypto.randomUUID();

// 用第一条用户消息的前若干字当会话标题
function titleFrom(text: string): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > 24 ? `${oneLine.slice(0, 24)}…` : oneLine;
}

// 侧栏分组：今天 / 昨天 / 更早
export function groupOf(session: Session): string {
  const now = new Date();
  const startOfToday = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
  ).getTime();
  if (session.updatedAt >= startOfToday) return "今天";
  if (session.updatedAt >= startOfToday - 24 * 60 * 60 * 1000) return "昨天";
  return "更早";
}

// 创建一个空会话
function emptySession(): Session {
  const now = Date.now();
  return {
    id: newId(),
    title: "新对话",
    messages: [],
    createdAt: now,
    updatedAt: now,
  };
}

// 取当前会话的消息（组件和 ipc 层都用它）
export function activeMessages(state: ChatState): UiMessage[] {
  return (
    state.sessions.find((s) => s.id === state.activeSessionId)?.messages ?? EMPTY
  );
}

// 把当前会话替换成新的一份，其它会话原样不动
function patchActive(
  s: ChatState,
  fn: (session: Session) => Session,
): Pick<ChatState, "sessions"> {
  return {
    sessions: s.sessions.map((session) =>
      session.id === s.activeSessionId ? fn(session) : session,
    ),
  };
}

// 把「含有指定消息的那个会话」替换成新的一份
// 为什么需要它：用户在流式输出过程中切到别的会话时，
// 文字片段必须继续写到原来那条消息上，而不是写进新会话。
// 用消息 id 反查会话，就不依赖「当前会话」这个会变的东西。
function patchByMessage(
  s: ChatState,
  messageId: string,
  fn: (session: Session) => Session,
): Pick<ChatState, "sessions"> {
  return {
    sessions: s.sessions.map((session) =>
      session.messages.some((m) => m.id === messageId) ? fn(session) : session,
    ),
  };
}

// 初始就准备一个空会话
const firstSession = emptySession();

export const useChatStore = create<ChatState>((set) => ({
  sessions: [firstSession],
  activeSessionId: firstSession.id,
  isStreaming: false,
  error: null,
  draft: "",

  setDraft: (text) => set({ draft: text }),

  // 新建一个会话并切过去
  newSession: () =>
    set((s) => {
      const session = emptySession();
      return {
        sessions: [session, ...s.sessions],
        activeSessionId: session.id,
        error: null,
        draft: "",
      };
    }),

  // 切换到某个会话
  // 注意：这里不碰 isStreaming。后端那一轮可能还在跑，
  // 把它置 false 会让用户以为没在生成，从而在同一会话里并发发起第二轮。
  selectSession: (id) => set({ activeSessionId: id, error: null, draft: "" }),

  // 删除会话；删掉的正好是当前会话时，自动切到剩下第一个
  deleteSession: (id) =>
    set((s) => {
      const rest = s.sessions.filter((x) => x.id !== id);
      // 全删光了，就补一个新的空会话
      if (rest.length === 0) {
        const fresh = emptySession();
        return { sessions: [fresh], activeSessionId: fresh.id };
      }
      return {
        sessions: rest,
        activeSessionId:
          s.activeSessionId === id ? rest[0].id : s.activeSessionId,
      };
    }),

  // 清空当前会话里的消息（会话本身留着）
  clearActiveSession: () =>
    set((s) => ({
      error: null,
      draft: "",
      sessions: s.sessions.map((session) =>
        session.id === s.activeSessionId
          ? { ...session, title: "新对话", messages: [], updatedAt: Date.now() }
          : session,
      ),
    })),

  // 用数据库读回来的会话列表替换内存列表
  // 只带元信息（标题、时间），消息要切到那个会话时再单独读
  hydrateSessions: (metas) =>
    set((s) => {
      // 数据库里一个会话都没有：保留内存里那个空的，别让界面空掉
      if (metas.length === 0) return s;
      // 把数据库里的会话转成内存里的会话对象
      const sessions: Session[] = metas.map((meta) => ({
        // id 直接沿用
        id: meta.id,
        // 标题沿用
        title: meta.title,
        // 消息先留空，等切过去再读
        messages: [],
        // 创建时间沿用
        createdAt: meta.created_at,
        // 更新时间沿用
        updatedAt: meta.updated_at,
      }));
      // 当前会话保持在列表里的第一个（数据库按更新时间倒序返回）
      const active = sessions.some((x) => x.id === s.activeSessionId)
        ? s.activeSessionId
        : sessions[0].id;
      // 整体替换
      return {
        // 换掉整个列表
        sessions,
        // 当前会话 id
        activeSessionId: active,
      };
    }),

  // 用数据库读回来的消息替换某个会话的消息
  setSessionMessages: (sessionId, messages) =>
    set((s) => ({
      // 只改这一个会话
      sessions: s.sessions.map((session) =>
        session.id === sessionId ? { ...session, messages } : session,
      ),
    })),

  // 删掉某条消息之后的所有消息（重新生成时把旧回复抹掉）
  truncateAfter: (sessionId, messageId) =>
    set((s) => ({
      // 只改这一个会话
      sessions: s.sessions.map((session) => {
        // 不是目标会话就跳过
        if (session.id !== sessionId) return session;
        // 找到那条消息的位置
        const index = session.messages.findIndex((m) => m.id === messageId);
        // 找不到就什么都不做
        if (index < 0) return session;
        // 只保留到那条消息为止（含它自己）
        return {
          ...session,
          messages: session.messages.slice(0, index + 1),
        };
      }),
    })),

  // 加一条用户消息；如果这是本会话第一条消息，用它当标题
  appendUser: (text) =>
    set((s) => ({
      ...patchActive(s, (session) => ({
        ...session,
        title:
          session.messages.length === 0 ? titleFrom(text) : session.title,
        messages: [
          ...session.messages,
          {
            id: newId(),
            role: "user",
            content: text,
            createdAt: Date.now(),
          },
        ],
        updatedAt: Date.now(),
      })),
      error: null,
    })),

  // 建一条空助手消息，返回它的 id
  // 同时把 isStreaming 置为 true：这一步是「正在生成」状态的唯一来源，
  // 停止按钮、发送禁用、重复发送保护全都看它
  startAssistant: () => {
    const id = newId();
    set((s) => ({
      // 标记为正在生成
      isStreaming: true,
      ...patchActive(s, (session) => ({
        ...session,
        messages: [
          ...session.messages,
          {
            id,
            role: "assistant",
            content: "",
            streaming: true,
            createdAt: Date.now(),
          },
        ],
        updatedAt: Date.now(),
      })),
    }));
    return id;
  },

  // 给 id 匹配的那条追加文字
  // 按消息 id 反查所属会话：用户中途切走也不会写错地方
  appendDelta: (id, text) =>
    set((s) =>
      patchByMessage(s, id, (session) => ({
        ...session,
        messages: session.messages.map((m) =>
          m.id === id ? { ...m, content: m.content + text } : m,
        ),
        updatedAt: Date.now(),
      })),
    ),

  // 为每个工具调用插一张卡片
  // anchorId 是这一轮那条助手消息的 id，用它定位该插到哪个会话里
  addToolCards: (calls, anchorId) =>
    set((s) =>
      patchByMessage(s, anchorId, (session) => ({
        ...session,
        messages: [
          ...session.messages,
          ...calls.map((call) => ({
            id: newId(),
            role: "tool" as const,
            content: "",
            toolName: call.function.name,
            toolCall: call,
            createdAt: Date.now(),
          })),
        ],
        updatedAt: Date.now(),
      })),
    ),

  // 结束生成：清掉 streaming 标记
  // 如果这条助手消息一个字都没收到（比如刚发出就被停了），把它删掉，
  // 否则界面上会留下一条空气泡
  // anchorId 用来定位「这一轮」属于哪个会话
  finish: (anchorId) =>
    set((s) => ({
      // 生成结束
      isStreaming: false,
      // 只动包含这条助手消息的会话
      sessions: s.sessions.map((session) => {
        // 找的是这条消息所在的会话
        const owns =
          anchorId !== undefined
            ? session.messages.some((m) => m.id === anchorId)
            : session.id === s.activeSessionId;
        // 不是目标会话就原样返回
        if (!owns) return session;
        // 处理这一个会话
        return {
          ...session,
          messages: session.messages
            // 删掉「正在流式且内容为空」的那一条
            .filter((m) => !(m.streaming && m.content === ""))
            // 其余的清掉 streaming 标记
            .map((m) => (m.streaming ? { ...m, streaming: false } : m)),
        };
      }),
    })),

  // 出错：记录错误，并把还没接到文字的空气泡删掉
  // anchorId 用来定位「这一轮」属于哪个会话
  fail: (message, anchorId) =>
    set((s) => ({
      // 生成结束
      isStreaming: false,
      // 记录错误
      error: message,
      // 只动包含这条助手消息的会话
      sessions: s.sessions.map((session) => {
        // 找的是这条消息所在的会话
        const owns =
          anchorId !== undefined
            ? session.messages.some((m) => m.id === anchorId)
            : session.id === s.activeSessionId;
        // 不是目标会话就原样返回
        if (!owns) return session;
        // 处理这一个会话
        return {
          ...session,
          messages: session.messages
            // 删掉空气泡
            .filter((m) => !(m.streaming && m.content === ""))
            // 其余的清掉 streaming 标记
            .map((m) => (m.streaming ? { ...m, streaming: false } : m)),
        };
      }),
    })),

  clearError: () => set({ error: null }),
}));
