import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Activity,
  ArrowDown,
  ArrowDownUp,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  CandlestickChart,
  Check,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  Command,
  Component,
  Expand,
  ExternalLink,
  GripVertical,
  LayoutDashboard,
  ListFilter,
  LockKeyhole,
  MoreHorizontal,
  PanelLeft,
  Plus,
  Search,
  SlidersHorizontal,
  Star,
  Undo2,
  UnlockKeyhole,
  Upload,
  X,
} from "lucide-react";
import Chart from "./Chart";
import {
  chartNode,
  direction,
  initialBoards,
  newId,
  panelNames,
  parseImport,
  previousClose,
  removePanel,
  signed,
  splitDock,
  stocks,
  type Board,
  type DockNode,
  type DockSide,
  type FloatingPanel,
  type PanelKind,
  type PreviewPanel,
  type Stock,
} from "./data";

const IconButton = ({
  label,
  children,
  onClick,
  className = "",
  disabled = false,
}: {
  label: string;
  children: ReactNode;
  onClick?: () => void;
  className?: string;
  disabled?: boolean;
}) => (
  <button
    type="button"
    className={`icon-button ${className}`}
    title={label}
    aria-label={label}
    onClick={onClick}
    disabled={disabled}
  >
    {children}
  </button>
);

function Sparkline({
  change = 1,
  seed = 0,
  width = 64,
}: {
  change?: number;
  seed?: number;
  width?: number;
}) {
  const values = Array.from(
    { length: 24 },
    (_, i) =>
      21 -
      (change >= 0 ? i * 0.52 : -i * 0.28) +
      Math.sin(i * 1.7 + seed) * 4 +
      Math.cos(i * 0.7 + seed) * 2,
  );
  return (
    <svg
      width={width}
      height="30"
      viewBox="0 0 64 30"
      className={`sparkline ${direction(change)}`}
      aria-hidden="true"
    >
      <path
        d={`M ${values.map((y, i) => `${(i * 64) / 23},${Math.max(3, Math.min(28, y))}`).join(" L ")}`}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.25"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function Modal({
  title,
  subtitle,
  onClose,
  children,
  wide = false,
}: {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const elements = () =>
      Array.from(
        ref.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input, textarea, select, [tabindex="0"]',
        ) ?? [],
      );
    const focusTarget =
      ref.current?.querySelector<HTMLElement>("[data-autofocus]") ??
      elements()[0];
    focusTarget?.focus();
    const key = (e: KeyboardEvent) => {
      if (e.isComposing || e.keyCode === 229) return;
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
      if (e.key === "Tab") {
        const items = elements();
        const first = items[0],
          last = items[items.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };
    const root = ref.current;
    root?.addEventListener("keydown", key);
    return () => {
      root?.removeEventListener("keydown", key);
      previous?.isConnected && previous.focus();
    };
  }, [onClose]);
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`modal ${wide ? "wide" : ""}`}
      >
        <div className="modal-heading">
          <div>
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          <IconButton label="关闭对话框" onClick={onClose}>
            <X size={18} />
          </IconButton>
        </div>
        {children}
      </div>
    </div>
  );
}

function Depth({
  stock,
  compact = false,
}: {
  stock: Stock;
  compact?: boolean;
}) {
  const last = previousClose(stock);
  return (
    <div className={`depth-body ${compact ? "compact" : ""}`}>
      <div className="table-labels depth-row">
        <span>档位</span>
        <span>价格</span>
        <span>委托量 / 手</span>
      </div>
      {[5, 4, 3, 2, 1].map((n, i) => (
        <div className="depth-row" key={`sell${n}`}>
          <span className="muted">卖 {n}</span>
          <span className={direction(stock.price + n * 0.01 - last)}>
            {(stock.price + n * 0.01).toFixed(2)}
          </span>
          <span className="depth-number">
            <i
              className="depth-bar sell"
              style={{ width: `${22 + ((i * 27 + 17) % 74)}%` }}
            />
            <span>{[1368, 826, 2451, 1832, 956][i].toLocaleString()}</span>
          </span>
        </div>
      ))}
      <div className="depth-spread">
        <span className={direction(stock.change)}>
          {stock.price.toFixed(2)}
        </span>
        <span>最新成交</span>
        <span className={direction(stock.change)}>{signed(stock.change)}%</span>
      </div>
      {[1, 2, 3, 4, 5].map((n, i) => (
        <div className="depth-row" key={`buy${n}`}>
          <span className="muted">买 {n}</span>
          <span className={direction(stock.price - n * 0.01 - last)}>
            {(stock.price - n * 0.01).toFixed(2)}
          </span>
          <span className="depth-number">
            <i
              className="depth-bar buy"
              style={{ width: `${38 + ((i * 31 + 12) % 57)}%` }}
            />
            <span>{[2156, 1647, 3208, 1876, 2413][i].toLocaleString()}</span>
          </span>
        </div>
      ))}
      <div className="pressure">
        <div>
          <span className="up">委买 62.8%</span>
          <span className="down">委卖 37.2%</span>
        </div>
        <i>
          <b />
        </i>
      </div>
    </div>
  );
}

function Trades({ stock }: { stock: Stock }) {
  return (
    <div className="trades-body">
      <div className="trade-row table-labels">
        <span>时间</span>
        <span>成交价</span>
        <span>量 / 手</span>
        <span>方向</span>
      </div>
      {Array.from({ length: 10 }, (_, i) => (
        <div className="trade-row" key={i}>
          <span className="muted">
            14:32:{String(56 - i * 3).padStart(2, "0")}
          </span>
          <span className={direction(stock.change)}>
            {(stock.price - (i % 3) * 0.01).toFixed(2)}
          </span>
          <span>{[186, 42, 391, 56, 128, 72, 641, 20, 156, 38][i]}</span>
          <span className={i % 3 ? "up" : "down"}>{i % 3 ? "B" : "S"}</span>
        </div>
      ))}
    </div>
  );
}

function Metrics({ stock }: { stock: Stock }) {
  const prev = previousClose(stock);
  return (
    <div className="metrics-grid">
      {[
        ["今开", (prev * 0.994).toFixed(2)],
        ["昨收", prev.toFixed(2)],
        ["最高", Math.max(prev * 1.025, stock.price).toFixed(2)],
        ["最低", (prev * 0.977).toFixed(2)],
        ["成交量", "40.73 万手"],
        ["成交额", "8.46 亿"],
        ["换手率", "15.21%"],
        ["量比", "2.34"],
        ["总市值", "58.69 亿"],
        ["流通市值", "58.69 亿"],
      ].map(([label, value], i) => (
        <div key={label}>
          <span>{label}</span>
          <b className={i < 4 ? direction(Number(value) - prev) : ""}>
            {value}
          </b>
        </div>
      ))}
    </div>
  );
}

const infoItems = [
  {
    time: "14:28",
    tag: "行业",
    title: "文化传媒板块活跃，数字内容与 AI 应用方向受到关注",
    source: "行业观察",
  },
  {
    time: "13:46",
    tag: "公告",
    title: "关于公司经营情况及近期业务进展的说明",
    source: "公司公告",
  },
  {
    time: "11:32",
    tag: "市场",
    title: "午间观察：两市成交温和放量，市场风格逐步切换",
    source: "市场速览",
  },
];

export default function App({
  embedded = false,
  active = true,
  stockRequest,
}: {
  embedded?: boolean;
  active?: boolean;
  stockRequest?: { code: string; sequence: number } | null;
}) {
  const [boards, setBoards] = useState<Board[]>(initialBoards);
  const [activeId, setActiveId] = useState("overview");
  const board = boards.find((b) => b.id === activeId)!;
  const stock = stocks.find((s) => s.code === board.selected) ?? stocks[0];
  const [modal, setModal] = useState<
    | "search"
    | "library"
    | "import"
    | "rename"
    | "newboard"
    | "guide"
    | "news"
    | null
  >(null);
  const [query, setQuery] = useState("");
  const [listQuery, setListQuery] = useState("");
  const [filter, setFilter] = useState<"all" | "SH" | "SZ">("all");
  const [sort, setSort] = useState(false);
  const [menu, setMenu] = useState(false);
  const [importText, setImportText] = useState("");
  const [draftName, setDraftName] = useState("");
  const [locked, setLocked] = useState(false);
  const [dense, setDense] = useState(false);
  const [dockMenu, setDockMenu] = useState<string | null>(null);
  const [toast, setToast] = useState<{
    text: string;
    undo?: () => void;
  } | null>(null);
  const [history, setHistory] = useState<string | null>(null);
  const [news, setNews] = useState(0);
  const [bottomTab, setBottomTab] = useState("news");
  const [rightTab, setRightTab] = useState("depth");
  const [listWidth, setListWidth] = useState(282);
  const [rightWidth, setRightWidth] = useState(270);
  const [focusChart, setFocusChart] = useState(false);
  const updateBoard = (patch: Partial<Board> | ((b: Board) => Board)) =>
    setBoards((bs) =>
      bs.map((b) =>
        b.id === activeId
          ? typeof patch === "function"
            ? patch(b)
            : { ...b, ...patch }
          : b,
      ),
    );
  const notify = (text: string, undo?: () => void) => setToast({ text, undo });
  const selectStock = (code: string) => updateBoard({ selected: code });
  const closeModal = useRef(() => setModal(null)).current;
  const lastStockRequest = useRef<number | null>(null);

  useEffect(() => {
    if (!stockRequest || lastStockRequest.current === stockRequest.sequence)
      return;
    lastStockRequest.current = stockRequest.sequence;
    setBoards((bs) =>
      bs.map((b) =>
        b.id === activeId ? { ...b, selected: stockRequest.code } : b,
      ),
    );
  }, [stockRequest, activeId]);

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), 7000);
    return () => clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!active || e.isComposing || e.keyCode === 229) return;
      if (
        !embedded &&
        (e.ctrlKey || e.metaKey) &&
        e.key.toLowerCase() === "k"
      ) {
        e.preventDefault();
        setQuery("");
        setModal("search");
      }
      if (e.key === "Escape" && !modal) {
        setHistory(null);
        setDockMenu(null);
        setMenu(false);
        setFocusChart(false);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [modal, embedded, active]);
  useEffect(() => {
    setListQuery("");
    setFilter("all");
    setMenu(false);
    setHistory(null);
    setDockMenu(null);
  }, [activeId]);

  const addPanel = (kind: PanelKind) => {
    const panel: FloatingPanel = {
      id: newId(),
      kind,
      name: panelNames[kind],
      members: stocks.map((s) => s.code),
      x: Math.max(
        70,
        Math.min(
          440 + board.floating.length * 24,
          window.innerWidth - (kind === "chart" ? 650 : 365),
        ),
      ),
      y: 180 + (board.floating.length % 5) * 24,
    };
    updateBoard((b) => ({ ...b, floating: [...b.floating, panel] }));
    setModal(null);
    notify(`已打开「${panel.name}」独立浮窗`);
  };
  const closeFloating = (panel: FloatingPanel) => {
    const targetId = activeId;
    updateBoard((b) => ({
      ...b,
      floating: b.floating.filter((p) => p.id !== panel.id),
    }));
    setDockMenu(null);
    notify(`已移除「${panel.name}」`, () =>
      setBoards((bs) =>
        bs.map((b) =>
          b.id === targetId ? { ...b, floating: [...b.floating, panel] } : b,
        ),
      ),
    );
  };
  const dockPanel = (panel: FloatingPanel, side: DockSide) => {
    if (locked) return;
    updateBoard((b) => ({
      ...b,
      floating: b.floating.filter((p) => p.id !== panel.id),
      dock: splitDock(b.dock, panel, side),
    }));
    setDockMenu(null);
    notify("组件已插入当前看板");
  };
  const detachPanel = (panel: PreviewPanel) => {
    if (locked) return;
    updateBoard((b) => ({
      ...b,
      dock: removePanel(b.dock, panel.id) ?? chartNode(),
      floating: [...b.floating, { ...panel, x: 430, y: 180 }],
    }));
  };
  const closeDocked = (panel: PreviewPanel) => {
    const oldDock = board.dock,
      targetId = activeId;
    updateBoard((b) => ({
      ...b,
      dock: removePanel(b.dock, panel.id) ?? chartNode(),
    }));
    notify(`已移除「${panel.name}」`, () =>
      setBoards((bs) =>
        bs.map((b) => (b.id === targetId ? { ...b, dock: oldDock } : b)),
      ),
    );
  };
  const resize = (event: React.PointerEvent, side: "left" | "right") => {
    if (locked) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const start = event.clientX,
      width = side === "left" ? listWidth : rightWidth;
    const el = event.currentTarget;
    const move = (e: Event) => {
      const x = (e as PointerEvent).clientX;
      const value = Math.max(
        220,
        Math.min(370, width + (side === "left" ? x - start : start - x)),
      );
      side === "left" ? setListWidth(value) : setRightWidth(value);
    };
    const end = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
  };

  const panelContent = (panel: PreviewPanel) => {
    if (panel.kind === "depth") return <Depth stock={stock} compact />;
    if (panel.kind === "trades") return <Trades stock={stock} />;
    if (panel.kind === "metrics") return <Metrics stock={stock} />;
    if (panel.kind === "chart")
      return (
        <Chart symbol={stock} mode={board.mode} onInspectDay={setHistory} />
      );
    return (
      <div className="mini-stock-list">
        {panel.members
          .map((code) => stocks.find((s) => s.code === code)!)
          .filter(Boolean)
          .map((s) => (
            <button
              key={s.code}
              className={s.code === stock.code ? "selected" : ""}
              onClick={() => selectStock(s.code)}
            >
              <span>
                {s.name}
                <small>{s.code}</small>
              </span>
              <span className={direction(s.change)}>
                {s.price.toFixed(2)}
                <small>{signed(s.change)}%</small>
              </span>
            </button>
          ))}
      </div>
    );
  };

  const renderDock = (node: DockNode, isRoot = true): ReactNode =>
    node.type === "split" ? (
      <div className={`dock-split ${node.axis}`} data-testid="dock-split">
        {node.children.map((child, i) => (
          <div
            className="dock-child"
            key={child.type === "leaf" ? child.panel.id : i}
          >
            {renderDock(child, false)}
          </div>
        ))}
      </div>
    ) : (
      <div
        className={`dock-panel ${node.panel.kind === "chart" ? "chart-panel" : ""}`}
        data-testid={`docked-${node.panel.kind}`}
      >
        {node.panel.kind === "chart" ? (
          <>
            <div className="chart-toolbar">
              <div className="period-tabs" role="tablist" aria-label="图表周期">
                {(["intraday", "daily"] as const).map((mode, i) => (
                  <button
                    key={mode}
                    role="tab"
                    aria-selected={board.mode === mode}
                    onClick={() => updateBoard({ mode })}
                    className={board.mode === mode ? "active" : ""}
                  >
                    {i ? "日 K" : "分时"}
                  </button>
                ))}
              </div>
              <span className="toolbar-divider" />
              <span className="chart-ma">
                {board.mode === "intraday" ? (
                  <>
                    <i className="line-blue" />
                    价格
                    <i className="line-gold" />
                    均价
                  </>
                ) : (
                  "均线 MA5 / MA10 / MA20"
                )}
              </span>
              <span className="toolbar-spacer" />
              <span className="adjust-label">
                {board.mode === "daily" ? "不复权 · 示例" : "当日分时"}
              </span>
              <IconButton
                label={focusChart ? "退出图表专注模式" : "图表专注模式"}
                onClick={() => setFocusChart(!focusChart)}
              >
                <Expand size={15} />
              </IconButton>
              {!isRoot && (
                <IconButton
                  label="弹出图表"
                  onClick={() => detachPanel(node.panel)}
                  disabled={locked}
                >
                  <ExternalLink size={14} />
                </IconButton>
              )}
            </div>
            <div className="chart-canvas">{panelContent(node.panel)}</div>
            <div className="chart-hint">
              <span>
                <span className="hint-dot" /> 单击查看数据 · 再次单击收起
                {board.mode === "daily" ? " · 双击查看当日分时" : ""}
              </span>
              <span>A 股 / CNY</span>
            </div>
          </>
        ) : (
          <>
            <div className="panel-heading">
              <span>
                <i className="link-dot" />
                {node.panel.name}
              </span>
              <div>
                <IconButton
                  label={`弹出${node.panel.name}`}
                  onClick={() => detachPanel(node.panel)}
                  disabled={locked}
                >
                  <ExternalLink size={14} />
                </IconButton>
                <IconButton
                  label={`关闭${node.panel.name}`}
                  onClick={() => closeDocked(node.panel)}
                >
                  <X size={14} />
                </IconButton>
              </div>
            </div>
            <div className="aux-panel-body">{panelContent(node.panel)}</div>
          </>
        )}
      </div>
    );

  let members = board.members
    .map((code) => stocks.find((s) => s.code === code)!)
    .filter(Boolean)
    .filter(
      (s) =>
        (filter === "all" || s.market === filter) &&
        (s.name.includes(listQuery) || s.code.includes(listQuery)),
    );
  if (sort) members = [...members].sort((a, b) => b.change - a.change);
  const searchResults = stocks.filter((s) =>
    `${s.name}${s.code}${s.market}`.toLowerCase().includes(query.toLowerCase()),
  );
  const importResult = parseImport(importText, board.members);
  const prev = previousClose(stock);

  return (
    <div
      className={`app-shell ${dense ? "dense" : ""} ${focusChart ? "focus-chart" : ""}`}
    >
      <aside className="nav-rail">
        <div className="brand-mark" aria-label="Tydaro">
          <i />
          <i />
          <i />
        </div>
        <nav>
          <button
            className="rail-item active"
            aria-label="行情工作台"
            onClick={() => setFocusChart(false)}
          >
            <CandlestickChart size={21} />
            <span>行情</span>
          </button>
          <button
            className="rail-item"
            aria-label="添加组件"
            onClick={() => setModal("library")}
          >
            <Component size={21} />
            <span>组件</span>
          </button>
          <button
            className="rail-item"
            aria-label="新建看板"
            onClick={() => {
              setDraftName("");
              setModal("newboard");
            }}
          >
            <LayoutDashboard size={20} />
            <span>看板</span>
          </button>
        </nav>
        <div className="rail-bottom">
          <IconButton
            label="切换紧凑密度"
            onClick={() => setDense(!dense)}
            className={dense ? "selected" : ""}
          >
            <SlidersHorizontal size={19} />
          </IconButton>
          <IconButton label="设计与使用说明" onClick={() => setModal("guide")}>
            <CircleHelp size={19} />
          </IconButton>
          <button
            className="avatar"
            onClick={() => setModal("guide")}
            aria-label="预览说明"
          >
            T
          </button>
        </div>
      </aside>
      <div className="app-main">
        <header className="app-header">
          <div className="wordmark">
            tydaro<span>WORKSPACE</span>
          </div>
          <button
            className="global-search"
            onClick={() => {
              setQuery("");
              setModal("search");
            }}
          >
            <Search size={15} />
            <span>搜索股票、代码</span>
            <kbd>Ctrl K</kbd>
          </button>
          <div className="header-right">
            <span className="demo-badge">
              <i />
              模拟行情
            </span>
            <span className="header-separator" />
            <IconButton label="查看预览说明" onClick={() => setModal("guide")}>
              <CircleHelp size={17} />
            </IconButton>
            <button className="preview-label" onClick={() => setModal("guide")}>
              设计预览 <ExternalLink size={12} />
            </button>
          </div>
        </header>
        <div className="board-bar">
          <div className="board-tabs" role="tablist" aria-label="行情看板">
            {boards.map((b) => (
              <button
                key={b.id}
                role="tab"
                aria-selected={b.id === activeId}
                className={`board-tab ${b.id === activeId ? "active" : ""}`}
                onClick={() => setActiveId(b.id)}
              >
                <LayoutDashboard size={14} />
                {b.name}
                {b.id === activeId && <i />}
              </button>
            ))}
            <IconButton
              label="新增看板"
              onClick={() => {
                setDraftName("");
                setModal("newboard");
              }}
            >
              <Plus size={16} />
            </IconButton>
          </div>
          <div className="board-actions">
            <button
              className={`text-button ${locked ? "teal" : ""}`}
              onClick={() => {
                setLocked(!locked);
                setDockMenu(null);
              }}
              title="锁定后禁止调整尺寸和插入组件"
            >
              {locked ? <LockKeyhole size={13} /> : <UnlockKeyhole size={13} />}
              {locked ? "布局已锁定" : "自由布局"}
            </button>
            <button
              className="add-component"
              onClick={() => setModal("library")}
            >
              <Plus size={14} />
              添加组件
            </button>
          </div>
        </div>
        <div className="market-strip">
          {[
            { name: "上证指数", value: "3,962.84", change: 0.38 },
            { name: "深证成指", value: "13,184.72", change: 0.72 },
            { name: "创业板指", value: "3,067.18", change: 1.26 },
            { name: "科创 50", value: "1,426.35", change: -0.21 },
          ].map((m, i) => (
            <div className="market-item" key={m.name}>
              <div>
                <span>{m.name}</span>
                <b className={direction(m.change)}>{m.value}</b>
              </div>
              <Sparkline change={m.change} seed={i * 3} />
              <strong className={direction(m.change)}>
                {signed(m.change)}%
              </strong>
            </div>
          ))}
          <div className="market-overview">
            <span>
              沪深成交额 <b>1.42 万亿</b>
            </span>
            <div>
              <span className="up">涨 3,246</span>
              <i />
              <span className="down">跌 1,982</span>
            </div>
          </div>
        </div>
        <main
          className="workspace"
          style={
            {
              "--list-width": `${listWidth}px`,
              "--right-width": `${rightWidth}px`,
            } as React.CSSProperties
          }
        >
          <section className="stock-panel">
            <div className="panel-heading">
              <span>
                <i className="link-dot" />
                {board.listName}
                <small>{board.members.length}</small>
              </span>
              <div>
                <IconButton
                  label="添加股票"
                  onClick={() => {
                    setQuery("");
                    setModal("search");
                  }}
                >
                  <Plus size={16} />
                </IconButton>
                <div className="menu-anchor">
                  <IconButton
                    label="股票分组菜单"
                    onClick={() => setMenu(!menu)}
                  >
                    <MoreHorizontal size={17} />
                  </IconButton>
                  {menu && (
                    <div className="dropdown list-menu">
                      <button
                        onClick={() => {
                          setDraftName(board.listName);
                          setModal("rename");
                          setMenu(false);
                        }}
                      >
                        重命名分组
                      </button>
                      <button
                        onClick={() => {
                          setImportText("");
                          setModal("import");
                          setMenu(false);
                        }}
                      >
                        <Upload size={13} />
                        从文本导入
                      </button>
                      <button
                        onClick={() => {
                          setSort(!sort);
                          setMenu(false);
                        }}
                      >
                        按涨幅{sort ? "恢复顺序" : "排序"}
                      </button>
                    </div>
                  )}
                </div>
              </div>
            </div>
            <div className="list-filter">
              <div className="small-tabs">
                {(["all", "SH", "SZ"] as const).map((f, i) => (
                  <button
                    key={f}
                    className={f === filter ? "active" : ""}
                    onClick={() => setFilter(f)}
                  >
                    {["全部", "沪市", "深市"][i]}
                  </button>
                ))}
              </div>
              <button
                className="filter-button"
                title="按涨幅排序"
                aria-label="按涨幅排序"
                onClick={() => setSort(!sort)}
              >
                <ArrowDownUp size={13} />
              </button>
            </div>
            <label className="list-search">
              <Search size={13} />
              <input
                placeholder="筛选当前分组"
                aria-label="筛选当前分组"
                value={listQuery}
                onChange={(e) => setListQuery(e.target.value)}
              />
            </label>
            <div className="stock-columns table-labels">
              <span>名称 / 代码</span>
              <span>最新价</span>
              <button onClick={() => setSort(!sort)}>
                涨跌幅 <ChevronDown size={11} />
              </button>
            </div>
            <div className="stock-rows">
              {members.map((s, i) => (
                <div
                  className={`stock-row ${s.code === stock.code ? "selected" : ""}`}
                  key={s.code}
                >
                  <button
                    className="stock-select"
                    aria-label={`选择${s.name}`}
                    aria-pressed={s.code === stock.code}
                    onClick={() => selectStock(s.code)}
                  >
                    <span className="stock-identity">
                      <strong>{s.name}</strong>
                      <small>
                        <i className={`exchange ${s.market.toLowerCase()}`}>
                          {s.market === "SH" ? "沪" : "深"}
                        </i>
                        {s.code}
                      </small>
                    </span>
                    <span className={`stock-price ${direction(s.change)}`}>
                      {s.price.toFixed(2)}
                      <Sparkline change={s.change} seed={i} width={50} />
                    </span>
                    <span className={`change-pill ${direction(s.change)}`}>
                      {signed(s.change)}%
                    </span>
                  </button>
                  <button
                    className="remove-stock"
                    aria-label={`移除${s.name}`}
                    title="从当前分组移除"
                    onClick={() => {
                      const targetId = activeId;
                      updateBoard({
                        members: board.members.filter((c) => c !== s.code),
                      });
                      notify(`已从「${board.listName}」移除${s.name}`, () =>
                        setBoards((bs) =>
                          bs.map((b) =>
                            b.id === targetId && !b.members.includes(s.code)
                              ? { ...b, members: [...b.members, s.code] }
                              : b,
                          ),
                        ),
                      );
                    }}
                  >
                    <X size={11} />
                  </button>
                </div>
              ))}
              {!members.length && (
                <div className="empty-state">
                  <Search size={25} />
                  <span>没有匹配的股票</span>
                  <button
                    onClick={() => {
                      setListQuery("");
                      setFilter("all");
                    }}
                  >
                    清除筛选
                  </button>
                </div>
              )}
            </div>
            <button
              className="import-link"
              onClick={() => {
                setImportText("");
                setModal("import");
              }}
            >
              <Upload size={13} />
              从文本批量导入
              <ChevronRight size={13} />
            </button>
            <div className="list-footer">
              <i className="link-dot" />
              联动组 1<span>独立股票分组</span>
            </div>
          </section>
          <div
            className="resize-handle left"
            role="separator"
            aria-label="调整股票列表宽度"
            aria-orientation="vertical"
            tabIndex={0}
            onPointerDown={(e) => resize(e, "left")}
            onKeyDown={(e) => {
              if (!locked && (e.key === "ArrowLeft" || e.key === "ArrowRight"))
                setListWidth((w) =>
                  Math.max(
                    220,
                    Math.min(370, w + (e.key === "ArrowRight" ? 10 : -10)),
                  ),
                );
            }}
          />
          <section className="center-workspace">
            <div className="quote-header">
              <div className="quote-name">
                <div>
                  <h1>{stock.name}</h1>
                  <span className="stock-code">
                    {stock.market} {stock.code}
                  </span>
                  <button
                    className={`star-button ${board.members.includes(stock.code) ? "saved" : ""}`}
                    aria-label={
                      board.members.includes(stock.code)
                        ? "从当前分组移除所选股票"
                        : "加入当前分组"
                    }
                    onClick={() =>
                      updateBoard({
                        members: board.members.includes(stock.code)
                          ? board.members.filter((c) => c !== stock.code)
                          : [...board.members, stock.code],
                      })
                    }
                  >
                    <Star
                      size={16}
                      fill={
                        board.members.includes(stock.code)
                          ? "currentColor"
                          : "none"
                      }
                    />
                  </button>
                </div>
                <span className="sector-tag">{stock.sector}</span>
                <span className="stock-session">沪深 A 股 · 连续竞价</span>
              </div>
              <div className={`quote-price ${direction(stock.change)}`}>
                <strong>{stock.price.toFixed(2)}</strong>
                <div>
                  <b>
                    {signed(stock.price - prev)}{" "}
                    <span>{signed(stock.change)}%</span>
                  </b>
                  <span className="quote-time">10/14 14:32:56 · 模拟</span>
                </div>
              </div>
              <div className="quote-stats">
                <div>
                  <span>今开</span>
                  <b className="down">{(prev * 0.994).toFixed(2)}</b>
                </div>
                <div>
                  <span>最高</span>
                  <b className="up">
                    {Math.max(stock.price, prev * 1.025).toFixed(2)}
                  </b>
                </div>
                <div>
                  <span>成交量</span>
                  <b>40.73 万</b>
                </div>
                <div>
                  <span>换手率</span>
                  <b>15.21%</b>
                </div>
              </div>
            </div>
            <div className="main-dock">{renderDock(board.dock)}</div>
            <section className="insights-panel">
              <div className="insights-tabs">
                <div role="tablist" aria-label="市场信息">
                  <button
                    role="tab"
                    aria-selected={bottomTab === "news"}
                    className={bottomTab === "news" ? "active" : ""}
                    onClick={() => setBottomTab("news")}
                  >
                    资讯与公告<span>3</span>
                  </button>
                  <button
                    role="tab"
                    aria-selected={bottomTab === "sectors"}
                    className={bottomTab === "sectors" ? "active" : ""}
                    onClick={() => setBottomTab("sectors")}
                  >
                    板块表现
                  </button>
                </div>
                <span>本地示例</span>
              </div>
              {bottomTab === "news" ? (
                <div className="news-list">
                  {infoItems.map((item, i) => (
                    <button
                      key={item.time}
                      className="news-item"
                      onClick={() => {
                        setNews(i);
                        setModal("news");
                      }}
                    >
                      <time>{item.time}</time>
                      <span className={`news-tag tag-${i}`}>{item.tag}</span>
                      <span className="news-title">
                        {i === 1 ? `${stock.name}：${item.title}` : item.title}
                      </span>
                      <span className="news-source">{item.source}</span>
                      <ChevronRight size={13} />
                    </button>
                  ))}
                </div>
              ) : (
                <div className="sector-performance">
                  {[
                    ["文化传媒", 4.86],
                    ["半导体", 3.21],
                    ["汽车整车", 2.68],
                    ["证券", 1.92],
                    ["银行", -0.62],
                    ["白酒", -0.89],
                  ].map(([name, change]) => (
                    <div key={name}>
                      <span>{name}</span>
                      <i
                        style={{
                          width: `${Math.abs(Number(change)) * 8 + 5}%`,
                        }}
                        className={direction(Number(change))}
                      />
                      <b className={direction(Number(change))}>
                        {signed(Number(change))}%
                      </b>
                    </div>
                  ))}
                </div>
              )}
            </section>
          </section>
          <div
            className="resize-handle right"
            role="separator"
            aria-label="调整盘口宽度"
            aria-orientation="vertical"
            tabIndex={0}
            onPointerDown={(e) => resize(e, "right")}
            onKeyDown={(e) => {
              if (!locked && (e.key === "ArrowLeft" || e.key === "ArrowRight"))
                setRightWidth((w) =>
                  Math.max(
                    220,
                    Math.min(370, w + (e.key === "ArrowLeft" ? 10 : -10)),
                  ),
                );
            }}
          />
          <aside className="right-panels">
            <section className="book-panel">
              <div className="panel-heading">
                <div
                  className="right-tabs"
                  role="tablist"
                  aria-label="盘口数据"
                >
                  <button
                    role="tab"
                    aria-selected={rightTab === "depth"}
                    className={rightTab === "depth" ? "active" : ""}
                    onClick={() => setRightTab("depth")}
                  >
                    买卖盘
                  </button>
                  <button
                    role="tab"
                    aria-selected={rightTab === "metrics"}
                    className={rightTab === "metrics" ? "active" : ""}
                    onClick={() => setRightTab("metrics")}
                  >
                    关键数据
                  </button>
                </div>
                <span className="level-badge">L2 示例</span>
              </div>
              {rightTab === "depth" ? (
                <Depth stock={stock} />
              ) : (
                <Metrics stock={stock} />
              )}
            </section>
            <section className="prints-panel">
              <div className="panel-heading">
                <span>成交明细</span>
                <span className="live-label">
                  <i />
                  14:32
                </span>
              </div>
              <Trades stock={stock} />
              <div className="trade-legend">
                <span>
                  <i className="up" />B 主动买入
                </span>
                <span>
                  <i className="down" />S 主动卖出
                </span>
              </div>
            </section>
          </aside>
        </main>
        <footer className="status-bar">
          <span>
            <i className="status-dot" />
            演示模式<span className="status-divider">/</span>
            所有数据均为本地模拟
          </span>
          <div>
            <span>布局保留至本次预览关闭</span>
            <span className="status-divider">|</span>
            <span>14:32:56 UTC+8</span>
            <span className="keyboard-hint">
              <Command size={11} /> K 搜索
            </span>
          </div>
        </footer>
      </div>

      {board.floating.map((panel) => (
        <Floating
          key={panel.id}
          panel={panel}
          locked={locked}
          onMove={(x, y) =>
            updateBoard((b) => ({
              ...b,
              floating: b.floating.map((p) =>
                p.id === panel.id ? { ...p, x, y } : p,
              ),
            }))
          }
          onClose={() => closeFloating(panel)}
          onDock={() => setDockMenu(dockMenu === panel.id ? null : panel.id)}
        >
          {dockMenu === panel.id && (
            <div className="dock-menu">
              <strong>插入主图区域</strong>
              <span>选择插入方向</span>
              <div>
                {(
                  [
                    { side: "left", label: "左侧", Icon: ArrowLeft },
                    { side: "top", label: "上方", Icon: ArrowUp },
                    { side: "bottom", label: "下方", Icon: ArrowDown },
                    { side: "right", label: "右侧", Icon: ArrowRight },
                  ] as const
                ).map(({ side, label, Icon }) => (
                  <button key={side} onClick={() => dockPanel(panel, side)}>
                    <Icon size={16} />
                    {label}
                  </button>
                ))}
              </div>
            </div>
          )}
          <div
            className={`floating-content ${panel.kind === "chart" ? "floating-chart" : ""}`}
          >
            {panelContent(panel)}
          </div>
          <div className="floating-footer">
            <i className="link-dot" />
            {stock.name} · 联动组 1<span>网页浮窗预览</span>
          </div>
        </Floating>
      ))}

      {modal === "search" && (
        <Modal
          title="查找股票"
          subtitle="输入代码或名称，选择后联动当前看板"
          onClose={closeModal}
        >
          <div className="dialog-search">
            <Search size={19} />
            <input
              data-autofocus
              aria-label="搜索股票"
              placeholder="例如 603598 或 引力传媒"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="search-results">
            {searchResults.map((s) => (
              <div className="search-result" key={s.code}>
                <button
                  onClick={() => {
                    selectStock(s.code);
                    setModal(null);
                  }}
                >
                  <span>
                    <strong>{s.name}</strong>
                    <small>
                      {s.market} {s.code} · {s.sector}
                    </small>
                  </span>
                  <span className={direction(s.change)}>
                    {s.price.toFixed(2)}
                    <small>{signed(s.change)}%</small>
                  </span>
                </button>
                <IconButton
                  label={
                    board.members.includes(s.code)
                      ? `${s.name}已在分组`
                      : `添加${s.name}到分组`
                  }
                  disabled={board.members.includes(s.code)}
                  onClick={() => {
                    updateBoard((b) => ({
                      ...b,
                      members: [...b.members, s.code],
                    }));
                    notify(`已添加${s.name}`);
                  }}
                >
                  {board.members.includes(s.code) ? (
                    <Check size={17} />
                  ) : (
                    <Plus size={17} />
                  )}
                </IconButton>
              </div>
            ))}
            {!searchResults.length && (
              <div className="empty-state">演示目录未收录该股票</div>
            )}
          </div>
          <div className="modal-footnote">
            演示目录 · 12 只 A 股样例 · 不连接真实行情
          </div>
        </Modal>
      )}
      {modal === "library" && (
        <Modal
          title="搭建你的看板"
          subtitle="从需要的信息开始，每个组件都可以自由组合。"
          onClose={closeModal}
          wide
        >
          <div className="library-note">
            <ExternalLink size={15} />
            <span>添加后先打开独立浮窗，再选择位置插入看板。</span>
          </div>
          <div className="component-grid">
            {(
              [
                {
                  kind: "chart",
                  Icon: CandlestickChart,
                  desc: "分时、日 K 与成交量，定位每一次价格变化。",
                  preview: "chart",
                },
                {
                  kind: "stocklist",
                  Icon: ListFilter,
                  desc: "独立的股票分组，让不同策略各有归属。",
                  preview: "list",
                },
                {
                  kind: "depth",
                  Icon: ArrowDownUp,
                  desc: "买卖委托与盘口深度，观察即时供需。",
                  preview: "depth",
                },
                {
                  kind: "trades",
                  Icon: Activity,
                  desc: "逐笔成交时间、价格和主动买卖方向。",
                  preview: "list",
                },
                {
                  kind: "metrics",
                  Icon: LayoutDashboard,
                  desc: "价格、量额、换手与估值，重要数据一目了然。",
                  preview: "metrics",
                },
              ] as const
            ).map(({ kind, Icon, desc, preview }) => (
              <button
                key={kind}
                className="component-card"
                onClick={() => addPanel(kind)}
              >
                <div className={`component-mini ${preview}`}>
                  {preview === "chart" ? (
                    <Sparkline change={3} width={140} />
                  ) : (
                    Array.from(
                      { length: preview === "metrics" ? 6 : 4 },
                      (_, i) => (
                        <i key={i} style={{ width: `${40 + i * 12}%` }} />
                      ),
                    )
                  )}
                </div>
                <div>
                  <Icon size={18} />
                  <strong>{panelNames[kind]}</strong>
                  <Plus size={17} />
                </div>
                <p>{desc}</p>
              </button>
            ))}
          </div>
          <div className="modal-footnote">
            当前看板：{board.name} · 浮窗关闭即移除，支持撤销
          </div>
        </Modal>
      )}
      {modal === "import" && (
        <Modal
          title="从文本导入股票"
          subtitle={`将识别到的股票追加到「${board.listName}」，不影响其他分组。`}
          onClose={closeModal}
          wide
        >
          <label className="input-label" htmlFor="import-text">
            股票代码或名称
          </label>
          <textarea
            id="import-text"
            data-autofocus
            maxLength={20000}
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder={
              "粘贴文本，用空格、逗号或换行分隔\n603598 001318\nSH600519 宁德时代"
            }
          />
          <div className="import-summary">
            <span>
              <b>{importResult.added.length}</b>可添加
            </span>
            <span>
              <b>{importResult.existing.length}</b>已在分组
            </span>
            <span>
              <b>{importResult.duplicates.length}</b>重复
            </span>
            <span className={importResult.unknown.length ? "gold" : ""}>
              <b>{importResult.unknown.length}</b>未识别
            </span>
          </div>
          {importResult.added.length > 0 && (
            <div className="import-chips">
              {importResult.added.map((s) => (
                <span key={s.code}>
                  {s.name}
                  <small>{s.code}</small>
                </span>
              ))}
            </div>
          )}
          {importResult.unknown.length > 0 && (
            <p className="import-warning">
              演示目录未收录：{importResult.unknown.slice(0, 12).join("、")}
              {importResult.unknown.length > 12 ? "…" : ""}
            </p>
          )}
          <p className="modal-footnote">
            预览识别 12 只样例股票；仅支持粘贴文本，最多 20,000 字符。
          </p>
          <div className="modal-actions">
            <button className="secondary-button" onClick={closeModal}>
              取消
            </button>
            <button
              className="primary-button"
              disabled={!importResult.added.length}
              onClick={() => {
                updateBoard({
                  members: [
                    ...board.members,
                    ...importResult.added.map((s) => s.code),
                  ],
                });
                notify(
                  `已添加 ${importResult.added.length} 只股票到「${board.listName}」`,
                );
                setModal(null);
              }}
            >
              <Plus size={14} />
              添加 {importResult.added.length} 只股票
            </button>
          </div>
        </Modal>
      )}
      {(modal === "rename" || modal === "newboard") && (
        <Modal
          title={modal === "rename" ? "重命名股票分组" : "创建新看板"}
          subtitle={
            modal === "rename"
              ? "每个股票列表都是一个独立分组。"
              : "从基础行情布局开始，自由添加和组合组件。"
          }
          onClose={closeModal}
        >
          <form
            onSubmit={(e) => {
              e.preventDefault();
              const name = draftName.trim();
              if (!name) return;
              if (modal === "rename") updateBoard({ listName: name });
              else {
                if (boards.some((b) => b.name === name)) {
                  notify("看板名称已存在");
                  return;
                }
                const id = newId();
                setBoards((bs) => [
                  ...bs,
                  {
                    id,
                    name,
                    listName: "股票列表",
                    members: [],
                    selected: stock.code,
                    mode: "intraday",
                    dock: chartNode(),
                    floating: [],
                  },
                ]);
                setActiveId(id);
              }
              setModal(null);
            }}
          >
            <label className="input-label" htmlFor="name">
              名称
            </label>
            <input
              id="name"
              className="name-input"
              data-autofocus
              maxLength={24}
              value={draftName}
              onChange={(e) => setDraftName(e.target.value)}
              placeholder={
                modal === "rename" ? "例如：核心观察" : "例如：新能源观察"
              }
              required
            />
            <div className="modal-actions">
              <button
                type="button"
                className="secondary-button"
                onClick={closeModal}
              >
                取消
              </button>
              <button className="primary-button" disabled={!draftName.trim()}>
                {modal === "rename" ? "保存名称" : "创建看板"}
              </button>
            </div>
          </form>
        </Modal>
      )}
      {modal === "guide" && (
        <Modal
          title="让每一次决策更清晰"
          subtitle="Tydaro / 原生桌面工作台设计预览"
          onClose={closeModal}
          wide
        >
          <div className="guide-intro">
            <div className="brand-mark">
              <i />
              <i />
              <i />
            </div>
            <p>
              信息足够丰富，视线始终有焦点。
              <br />
              <span>浅色、高密度、可组合，为长时间看盘设计。</span>
            </p>
          </div>
          <div className="guide-grid">
            <div>
              <b>01 / 看板围绕任务</b>
              <p>
                切换 A
                股全景、短线观察、趋势研究。每个看板拥有独立分组、选股和组件布局。
              </p>
            </div>
            <div>
              <b>02 / 图表服务判断</b>
              <p>
                固定当日时轴，区分竞价、连续交易和午休。单击检视，日 K
                双击查看历史分时。
              </p>
            </div>
            <div>
              <b>03 / 组件自由组合</b>
              <p>
                新增即浮窗，拖动标题移动，用插入菜单选择四向位置。关闭即移除，可撤销。
              </p>
            </div>
            <div>
              <b>04 / 原生实现优先</b>
              <p>
                Flex 分栏、矢量图标、路径图表、普通浮层均可映射到
                GPUI，不依赖网页滤镜。
              </p>
            </div>
          </div>
          <div className="guide-boundary">
            当前为 React
            交互样机，行情、资讯与历史数据均为模拟。网页浮窗不等同于系统窗口，布局仅在本次会话保留。完整跨窗拖放、历史
            OHLC 一致性与 Windows DPI 需在原生版本验收。
          </div>
        </Modal>
      )}
      {modal === "news" && (
        <Modal
          title={
            infoItems[news].tag === "公告"
              ? `${stock.name} · 公告示例`
              : infoItems[news].source
          }
          subtitle={`2026/10/14 ${infoItems[news].time} · 本地模拟内容`}
          onClose={closeModal}
        >
          <article className="news-article">
            <h3>{infoItems[news].title}</h3>
            <p>
              这是一条用于展示阅读层级和交互方式的本地资讯样例，不代表真实新闻或公司公告。
            </p>
            <p>
              后续接入数据源时，这一区域可以承载新闻正文、关联股票、来源和发布时间，保留工作区上下文。
            </p>
          </article>
        </Modal>
      )}
      {history && (
        <Modal
          title={`${stock.name} · 历史分时`}
          subtitle={`${history} · 静态模拟会话（视觉示例，不校准日 K 聚合）`}
          onClose={() => setHistory(null)}
          wide
        >
          <div className="historical-toolbar">
            <span className="blue">{stock.code} / 当日分时</span>
            <span>单击查看分钟详情</span>
          </div>
          <div className="historical-chart">
            <Chart symbol={stock} mode="intraday" historical />
          </div>
        </Modal>
      )}
      {toast && (
        <div className="toast" role="status">
          <Check size={16} />
          <span>{toast.text}</span>
          {toast.undo && (
            <button
              onClick={() => {
                toast.undo?.();
                setToast(null);
              }}
            >
              <Undo2 size={13} />
              撤销
            </button>
          )}
          <IconButton label="关闭提示" onClick={() => setToast(null)}>
            <X size={14} />
          </IconButton>
        </div>
      )}
    </div>
  );
}

function Floating({
  panel,
  children,
  locked,
  onMove,
  onClose,
  onDock,
}: {
  panel: FloatingPanel;
  children: ReactNode;
  locked: boolean;
  onMove: (x: number, y: number) => void;
  onClose: () => void;
  onDock: () => void;
}) {
  const [dragging, setDragging] = useState(false);
  const windowRef = useRef<HTMLElement>(null);
  useEffect(() => {
    windowRef.current?.focus({ preventScroll: true });
  }, []);
  const drag = useRef<{
    x: number;
    y: number;
    startX: number;
    startY: number;
  } | null>(null);
  return (
    <section
      ref={windowRef}
      tabIndex={-1}
      className={`floating-window ${panel.kind === "chart" ? "wide-float" : ""} ${dragging ? "dragging" : ""}`}
      style={{
        left: panel.x,
        top: panel.y,
        maxWidth: `calc(100vw - ${panel.x + 8}px)`,
        maxHeight: `calc(100dvh - ${panel.y + 8}px)`,
      }}
      data-testid="floating-panel"
      aria-label={`${panel.name}独立浮窗`}
    >
      <div
        className="floating-title"
        onPointerDown={(e) => {
          if ((e.target as HTMLElement).closest("button")) return;
          drag.current = {
            x: panel.x,
            y: panel.y,
            startX: e.clientX,
            startY: e.clientY,
          };
          e.currentTarget.setPointerCapture(e.pointerId);
          setDragging(true);
        }}
        onPointerMove={(e) => {
          if (!drag.current) return;
          onMove(
            Math.max(
              70,
              Math.min(
                window.innerWidth - (windowRef.current?.offsetWidth ?? 355) - 8,
                drag.current.x + e.clientX - drag.current.startX,
              ),
            ),
            Math.max(
              52,
              Math.min(
                window.innerHeight -
                  (windowRef.current?.offsetHeight ?? 440) -
                  8,
                drag.current.y + e.clientY - drag.current.startY,
              ),
            ),
          );
        }}
        onPointerUp={() => {
          drag.current = null;
          setDragging(false);
        }}
        onPointerCancel={() => {
          drag.current = null;
          setDragging(false);
        }}
      >
        <span>
          <GripVertical size={14} />
          <i className="link-dot" />
          {panel.name}
        </span>
        <div>
          <button className="dock-button" disabled={locked} onClick={onDock}>
            <PanelLeft size={13} />
            插入面板
          </button>
          <IconButton label={`关闭独立${panel.name}`} onClick={onClose}>
            <X size={16} />
          </IconButton>
        </div>
      </div>
      {children}
    </section>
  );
}
