// ============================================================================
// 侧栏宽度：拖右缘改宽，并排时把主栏挤开；宽度记在 localStorage
//
// 抽屉（≤640px）不用这个宽度。CSS 把抽屉固定成 248px，拖动手柄也藏起来。
// 记下来的数还在，窗口再变宽就恢复。
//
// 拖的过程里不要 setState。每一格指针移动都重绘整棵树会卡，
// 流式输出时消息列表本来也在重绘。宽度直接写到 .app 的 CSS 变量上，松手才提交。
// ============================================================================

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
  RefObject,
} from "react";

// 默认跟改版前的固定宽度一样，没拖过的人看起来不应有变化
export const SIDEBAR_DEFAULT_WIDTH = 248;
// 再窄的话「新建对话」、会话标题和底栏「本地用户 / 设置」会挤坏
export const SIDEBAR_MIN_WIDTH = 180;
// 再宽会话栏就空了一大片，主栏阅读区也被挤瘦
export const SIDEBAR_MAX_WIDTH = 400;
// 并排时主栏至少留这么宽。248px 侧栏在 640px 窗口里大约只剩 390px，
// 再窄顶栏（模型名 + 清空对话）和输入区会挤坏。
export const MAIN_MIN_WIDTH = 400;

export const SIDEBAR_WIDTH_STORAGE_KEY = "mini_agent.sidebarWidth";

const KEYBOARD_STEP = 8;
const KEYBOARD_STEP_LARGE = 24;

// 用户选的宽度：只夹在绝对上下限里，不看当前窗口。
// 窗口暂时很窄时画面上会再收一截，但 localStorage 里仍留着这个数。
export function clampPreferredSidebarWidth(value: number): number {
  if (!Number.isFinite(value)) return SIDEBAR_DEFAULT_WIDTH;
  return Math.min(
    SIDEBAR_MAX_WIDTH,
    Math.max(SIDEBAR_MIN_WIDTH, Math.round(value)),
  );
}

// 当前窗口下侧栏最多能到多宽，主栏才不会被挤到 MAIN_MIN_WIDTH 以下
export function maxSidebarWidthForViewport(viewportWidth: number): number {
  if (!Number.isFinite(viewportWidth)) return SIDEBAR_MAX_WIDTH;
  const room = Math.floor(viewportWidth) - MAIN_MIN_WIDTH;
  return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, room));
}

// 真正画到屏幕上的宽度：用户的选择，再按当前窗口收一刀
export function clampSidebarWidth(value: number, viewportWidth: number): number {
  return Math.min(
    clampPreferredSidebarWidth(value),
    maxSidebarWidthForViewport(viewportWidth),
  );
}

// 窗口宽度是外部状态。用 useSyncExternalStore，避免 effect 里 setState 在并发渲染下读到旧值。
function subscribeViewport(onStoreChange: () => void): () => void {
  window.addEventListener("resize", onStoreChange);
  return () => window.removeEventListener("resize", onStoreChange);
}

function getViewportWidth(): number {
  return window.innerWidth;
}

function readPreferred(): number {
  try {
    const raw = window.localStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY);
    if (raw == null || raw.trim() === "") return SIDEBAR_DEFAULT_WIDTH;
    return clampPreferredSidebarWidth(Number(raw));
  } catch {
    // 隐私模式或 webview 禁掉存储时，退回默认宽度
    return SIDEBAR_DEFAULT_WIDTH;
  }
}

function writePreferred(width: number): void {
  try {
    window.localStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(width));
  } catch {
    // 写不进去就只留在内存里，这次会话仍然能拖
  }
}

// 收起时宽度和负 margin 必须同一帧一起变。
// 宽度本身没有过渡，margin 有：只改宽度的话，过渡中的 margin 还停在旧值，
// 主栏会被拽出屏幕一截。先关掉过渡，把新值落到布局上，再把过渡开回来。
function applyWidth(app: HTMLDivElement | null, next: number): void {
  if (!app) return;
  const px = `${next}px`;
  if (!app.classList.contains("app--collapsed")) {
    app.style.setProperty("--sidebar-width", px);
    return;
  }
  app.classList.add("app--no-sidebar-transition");
  app.style.setProperty("--sidebar-width", px);
  void app.offsetWidth;
  app.classList.remove("app--no-sidebar-transition");
}

export function useSidebarWidth(appRef: RefObject<HTMLDivElement | null>) {
  const [preferred, setPreferred] = useState(readPreferred);
  const viewportWidth = useSyncExternalStore(
    subscribeViewport,
    getViewportWidth,
  );
  // 拖动中为 true。这段时间布局效果不要用 state 里的旧宽度盖掉手指下的宽度
  const draggingRef = useRef(false);
  // 卸载时拆掉还挂在 window 上的指针监听
  const endDragRef = useRef<(() => void) | null>(null);
  const width = clampSidebarWidth(preferred, viewportWidth);
  const max = maxSidebarWidthForViewport(viewportWidth);
  // 指针按下时读这个，不在渲染期间写 ref
  const widthRef = useRef(width);

  useLayoutEffect(() => {
    widthRef.current = width;
    if (draggingRef.current) return;
    applyWidth(appRef.current, width);
  }, [appRef, width]);

  useEffect(() => {
    return () => {
      endDragRef.current?.();
      endDragRef.current = null;
      draggingRef.current = false;
      document.body.classList.remove("sidebar-resizing");
    };
  }, []);

  // 松手或键盘才写 localStorage。窗口自己变窄时不写，免得把用户拉宽后的选择冲掉
  const commit = useCallback((next: number) => {
    const stored = clampPreferredSidebarWidth(next);
    setPreferred(stored);
    writePreferred(stored);
  }, []);

  const onPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (event.button !== 0 || draggingRef.current) return;
      const handle = event.currentTarget;
      const app = appRef.current;
      const pointerId = event.pointerId;
      // 别抢走输入框的焦点，也别让浏览器选中文字
      event.preventDefault();
      try {
        handle.setPointerCapture(pointerId);
      } catch {
        // 指针已经失效时捕获会抛，下面的 window 监听仍然能跟着走
      }

      const startX = event.clientX;
      const startWidth = widthRef.current;
      let latest = startWidth;
      let moved = false;
      let settled = false;
      draggingRef.current = true;
      document.body.classList.add("sidebar-resizing");

      const stopListening = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", finish);
        window.removeEventListener("pointercancel", finish);
        draggingRef.current = false;
        document.body.classList.remove("sidebar-resizing");
        if (handle.hasPointerCapture(pointerId)) {
          handle.releasePointerCapture(pointerId);
        }
        if (endDragRef.current === stopListening) endDragRef.current = null;
      };
      endDragRef.current = stopListening;

      const move = (ev: PointerEvent) => {
        if (ev.pointerId !== pointerId) return;
        const delta = ev.clientX - startX;
        if (!moved && Math.abs(delta) < 1) return;
        moved = true;
        latest = clampSidebarWidth(startWidth + delta, window.innerWidth);
        applyWidth(app, latest);
        handle.setAttribute("aria-valuenow", String(latest));
      };

      const finish = (ev: PointerEvent) => {
        if (ev.pointerId !== pointerId || settled) return;
        settled = true;
        // pointercancel 的坐标经常是 0，用最后一次 move 的宽度，别跳回屏幕左上角
        if (ev.type === "pointerup") {
          const delta = ev.clientX - startX;
          if (Math.abs(delta) >= 1) {
            moved = true;
            latest = clampSidebarWidth(startWidth + delta, window.innerWidth);
            applyWidth(app, latest);
          }
        }
        stopListening();
        // 只点了一下、没有拖：别把「窗口暂时收窄」的宽度写进 localStorage
        if (moved) commit(latest);
      };

      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", finish);
      window.addEventListener("pointercancel", finish);
    },
    [appRef, commit],
  );

  const onKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      const step = event.shiftKey ? KEYBOARD_STEP_LARGE : KEYBOARD_STEP;
      let next: number | null = null;
      if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
        next = widthRef.current - step;
      } else if (event.key === "ArrowRight" || event.key === "ArrowUp") {
        next = widthRef.current + step;
      } else if (event.key === "Home") {
        next = SIDEBAR_MIN_WIDTH;
      } else if (event.key === "End") {
        next = maxSidebarWidthForViewport(window.innerWidth);
      }
      if (next == null) return;
      event.preventDefault();
      const applied = clampSidebarWidth(next, window.innerWidth);
      applyWidth(appRef.current, applied);
      commit(applied);
    },
    [appRef, commit],
  );

  return {
    width,
    min: SIDEBAR_MIN_WIDTH,
    max,
    onPointerDown,
    onKeyDown,
  };
}
