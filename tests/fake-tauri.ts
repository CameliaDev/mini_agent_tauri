// 测试专用的 invoke 替身。只实现会话读写，让 sessions.ts 不连真 Tauri。

export interface FakeSession {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
}

export interface FakeMessage {
  id: string;
  role: string;
  content: string;
  tool_name: string | null;
  tool_call: unknown;
  created_at: number;
  seq: number;
  sessionId: string;
}

let sessions = new Map<string, FakeSession>();
let messages: FakeMessage[] = [];
let blocker: Promise<void> = Promise.resolve();
let writesStarted = 0;

export function __resetDb(): void {
  sessions = new Map();
  messages = [];
  blocker = Promise.resolve();
  writesStarted = 0;
}

export function __messages(sessionId: string): FakeMessage[] {
  return messages
    .filter((message) => message.sessionId === sessionId)
    .sort((a, b) => a.seq - b.seq)
    .map((message) => ({ ...message }));
}

export function __writesStarted(): number {
  return writesStarted;
}

// 下一笔 save / append 会停在这里，用来制造「旧落库晚于新落库」的交错
export function __blockWrites(): { release: () => void } {
  let release!: () => void;
  blocker = new Promise((resolve) => {
    release = () => {
      blocker = Promise.resolve();
      resolve();
    };
  });
  return { release };
}

export class Channel<T> {
  constructor(public callback: (payload: T) => void) {}
}

export async function invoke(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<unknown> {
  if (cmd === "save_session" || cmd === "append_message") {
    writesStarted += 1;
    await blocker;
  }
  switch (cmd) {
    case "list_sessions":
      return [...sessions.values()].sort((a, b) => b.updated_at - a.updated_at);
    case "load_messages":
      return __messages(String(args.sessionId)).map(
        ({ seq: _seq, sessionId: _sessionId, ...rest }) => rest,
      );
    case "save_session": {
      const session = args.session as FakeSession;
      sessions.set(session.id, { ...session });
      return null;
    }
    case "append_message": {
      const message = args.message as Omit<FakeMessage, "seq" | "sessionId">;
      const row: FakeMessage = {
        ...message,
        seq: Number(args.seq),
        sessionId: String(args.sessionId),
      };
      const index = messages.findIndex((item) => item.id === row.id);
      if (index >= 0) messages[index] = row;
      else messages.push(row);
      return null;
    }
    case "delete_session":
      sessions.delete(String(args.sessionId));
      messages = messages.filter(
        (message) => message.sessionId !== String(args.sessionId),
      );
      return null;
    case "clear_session_messages":
      messages = messages.filter(
        (message) => message.sessionId !== String(args.sessionId),
      );
      return null;
    default:
      throw new Error(`unexpected command ${cmd}`);
  }
}
