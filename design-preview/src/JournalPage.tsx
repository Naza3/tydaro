import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ArrowDownLeft,
  ArrowUpRight,
  BookOpen,
  CalendarDays,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock3,
  FilePenLine,
  Grip,
  History,
  Maximize2,
  NotebookPen,
  Pencil,
  Plus,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { newId, stocks } from "./data";
import {
  DEMO_DATE,
  pageNames,
  type DraftRequest,
  type SourceContext,
} from "./workspace-model";
import "./JournalPage.css";

type Plan = {
  id: string;
  sector: string;
  basis: string;
  codes: string[];
  method: string;
  entry: string;
  avoid: string;
  sellMethod: string;
  exit: string;
  hold: string;
  source: string;
};
type Draft = Plan & { editingId?: string; requestId?: string };
type Day = { plans: Plan[]; drafts: Draft[]; note: string };
type Hot = {
  id: string;
  date: string;
  sector: string;
  heights: string;
  created: number;
};
type Buy = {
  id: string;
  date: string;
  stock: string;
  sector: string;
  method: string;
  profit: number;
  note: string;
};
type Journal = {
  version: 2;
  days: Record<string, Day>;
  hotspots: Hot[];
  buys: Buy[];
  methods: string[];
  sellMethods: string[];
};
type Props = {
  incoming: DraftRequest | null;
  onInspect: (code: string, context?: SourceContext) => void;
  onAddToWatch: (context: SourceContext) => void;
  onNotice: (message: string) => void;
  onSaveStatus?: (status: string) => void;
};
const KEY = "tydaro.preview.journal.v2";
const blankDay = (): Day => ({ plans: [], drafts: [], note: "" });
const blankPlan = (): Draft => ({
  id: newId(),
  sector: "",
  basis: "",
  codes: [],
  method: "",
  entry: "",
  avoid: "",
  sellMethod: "",
  exit: "",
  hold: "",
  source: "手工编写",
});
const sortHot = (items: Hot[]) =>
  [...items].sort(
    (a, b) => b.date.localeCompare(a.date) || b.created - a.created,
  );
const sortBuy = (items: Buy[]) =>
  [...items].sort((a, b) => b.date.localeCompare(a.date));
const dateLabel = (date: string) => date.slice(5).replace("-", "/");
const dateValid = (value: string) =>
  /^\d{4}-\d{2}-\d{2}$/.test(value) &&
  !Number.isNaN(Date.parse(value)) &&
  new Date(value).toISOString().slice(0, 10) === value;
const emptySide = (values: string[]) => values.every((value) => !value.trim());

function seedJournal(): Journal {
  const media: Plan = {
    id: "plan-media",
    sector: "文化传媒",
    basis:
      "核心股封板后观察板块跟随，优先确认分歧承接与量能，不根据单一涨幅追涨。",
    codes: ["603598"],
    method: "分歧转一致",
    entry: "板块保持活跃，核心股回踩均价线后放量承接，再观察确认。",
    avoid: "板块回落、核心股放量跌破均价线，或开盘直接加速。",
    sellMethod: "冲高分批",
    exit: "冲高量价背离，或板块整体转弱时按预案处理。",
    hold: "核心股承接持续，板块梯队保持完整。",
    source: "题材研究 · 10/14",
  };
  const tech: Plan = {
    id: "plan-tech",
    sector: "半导体",
    basis: "趋势观察：以支撑是否有效作为判断依据，等待走势确认。",
    codes: ["688981", "002475"],
    method: "",
    entry: "",
    avoid: "",
    sellMethod: "跌破支撑",
    exit: "放量跌破前一日低点，反抽未能收回时复核离场条件。",
    hold: "缩量调整，仍在关键支撑之上；记录下一次确认点。",
    source: "手工编写",
  };
  const previous = {
    ...media,
    id: "plan-previous",
    sector: "汽车整车",
    codes: ["601127"],
    source: "手工编写",
    basis: "观察板块轮动中的承接强度，记录分歧后的量价表现。",
  };
  const buys: Buy[] = [
    ["2026-10-14", "引力传媒", "文化传媒", 12.8],
    ["2026-10-13", "赛力斯", "汽车整车", 8.6],
    ["2026-10-12", "中芯国际", "半导体", 10],
    ["2026-10-09", "东方财富", "证券", -2.4],
    ["2026-10-08", "立讯精密", "消费电子", 5.2],
    ["2026-09-30", "阳光乳业", "食品饮料", 3.1],
    ["2026-09-29", "比亚迪", "汽车整车", -1.6],
    ["2026-09-28", "美的集团", "家用电器", 2.7],
    ["2026-09-25", "宁德时代", "电池", 6.4],
    ["2026-09-24", "招商银行", "银行", 0.8],
    ["2026-09-01", "五粮液", "白酒", 25.2],
  ].map(([date, stock, sector, profit], index) => ({
    id: `buy-${index}`,
    date: String(date),
    stock: String(stock),
    sector: String(sector),
    profit: Number(profit),
    method: index % 2 ? "趋势回踩" : "分歧转一致",
    note: "手工录入的演示记录；收益率不代表账户核算。",
  }));
  return {
    version: 2,
    days: {
      [DEMO_DATE]: {
        plans: [media, tech],
        drafts: [],
        note: "10:06  传媒板块保持活跃，先观察核心股回封后的承接，不追开盘加速。\n10:24  半导体出现分化，继续跟踪关键支撑，暂不增加新的买入计划。",
      },
      "2026-10-13": {
        plans: [previous],
        drafts: [],
        note: "分歧时观察成交量，收盘后补充对照预案的执行记录。",
      },
      "2026-10-12": {
        plans: [{ ...tech, id: "plan-oct12" }],
        drafts: [],
        note: "保持观察，等待条件充分。",
      },
    },
    hotspots: [
      {
        id: "hot-media",
        date: DEMO_DATE,
        sector: "文化传媒",
        heights: "引力传媒 · 4 板\n板块核心承接观察",
        created: 5,
      },
      {
        id: "hot-chip",
        date: DEMO_DATE,
        sector: "半导体",
        heights: "中芯国际 · 趋势观察\n人工记录，不自动查询行情",
        created: 4,
      },
      {
        id: "hot-auto",
        date: "2026-10-13",
        sector: "汽车整车",
        heights: "赛力斯 · 核心观察",
        created: 3,
      },
      {
        id: "hot-consumer",
        date: "2026-10-12",
        sector: "消费电子",
        heights: "立讯精密 · 趋势观察",
        created: 2,
      },
    ],
    buys,
    methods: [],
    sellMethods: [],
  };
}

function validPlan(value: unknown): value is Plan {
  if (!value || typeof value !== "object") return false;
  const plan = value as Record<string, unknown>;
  return (
    [
      "id",
      "sector",
      "basis",
      "method",
      "entry",
      "avoid",
      "sellMethod",
      "exit",
      "hold",
      "source",
    ].every((key) => typeof plan[key] === "string") &&
    Array.isArray(plan.codes) &&
    plan.codes.every((code) => typeof code === "string")
  );
}

function loadJournal(): {
  data: Journal;
  mode: "local" | "session" | "corrupt";
} {
  let raw: string | null;
  try {
    raw = localStorage.getItem(KEY);
  } catch {
    return { data: seedJournal(), mode: "session" };
  }
  if (!raw) return { data: seedJournal(), mode: "local" };
  try {
    const value = JSON.parse(raw) as Journal;
    if (
      value.version !== 2 ||
      !value.days ||
      typeof value.days !== "object" ||
      Array.isArray(value.days) ||
      !Array.isArray(value.hotspots) ||
      !Array.isArray(value.buys) ||
      !Array.isArray(value.methods) ||
      !Array.isArray(value.sellMethods)
    )
      throw new Error("shape");
    if (
      !Object.entries(value.days).every(
        ([date, day]) =>
          dateValid(date) &&
          day &&
          Array.isArray(day.plans) &&
          day.plans.every(validPlan) &&
          Array.isArray(day.drafts) &&
          day.drafts.every(validPlan) &&
          typeof day.note === "string",
      )
    )
      throw new Error("days");
    if (
      value.hotspots.length > 5 ||
      !value.hotspots.every(
        (hot) =>
          hot &&
          typeof hot.id === "string" &&
          dateValid(hot.date) &&
          typeof hot.sector === "string" &&
          typeof hot.heights === "string" &&
          Number.isFinite(hot.created),
      )
    )
      throw new Error("hotspots");
    if (
      !value.buys.every(
        (buy) =>
          buy &&
          [buy.id, buy.stock, buy.sector, buy.method, buy.note].every(
            (item) => typeof item === "string",
          ) &&
          dateValid(buy.date) &&
          Number.isFinite(buy.profit),
      )
    )
      throw new Error("buys");
    if (
      ![...value.methods, ...value.sellMethods].every(
        (item) => typeof item === "string",
      )
    )
      throw new Error("methods");
    return { data: value, mode: "local" };
  } catch {
    return { data: seedJournal(), mode: "corrupt" };
  }
}

function JournalDialog({
  title,
  children,
  onClose,
  className = "",
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const dialog = ref.current;
    const frame = requestAnimationFrame(() =>
      dialog
        ?.querySelector<HTMLElement>("input, textarea, select, button")
        ?.focus(),
    );
    return () => {
      cancelAnimationFrame(frame);
      previous?.focus();
    };
  }, []);
  return (
    <div className="journal-modal-backdrop">
      <div
        ref={ref}
        className={`journal-modal ${className}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing || event.keyCode === 229) return;
          if (event.key === "Escape") {
            event.stopPropagation();
            close.current();
          }
          if (event.key === "Tab") {
            const items = [
              ...(ref.current?.querySelectorAll<HTMLElement>(
                'button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex="0"]',
              ) ?? []),
            ].filter((item) => item.getClientRects().length);
            if (!items.length) return;
            if (event.shiftKey && document.activeElement === items[0]) {
              event.preventDefault();
              items.at(-1)?.focus();
            } else if (
              !event.shiftKey &&
              document.activeElement === items.at(-1)
            ) {
              event.preventDefault();
              items[0].focus();
            }
          }
        }}
      >
        <header className="journal-modal-title">
          <h2>{title}</h2>
          <button
            className="journal-icon-button"
            aria-label={`关闭${title}`}
            onClick={onClose}
          >
            <X size={18} />
          </button>
        </header>
        {children}
      </div>
    </div>
  );
}

export default function JournalPage({
  incoming,
  onInspect,
  onAddToWatch,
  onNotice,
  onSaveStatus,
}: Props) {
  const [initial] = useState(loadJournal);
  const [data, setData] = useState(initial.data);
  const [mode, setMode] = useState(initial.mode);
  const [date, setDate] = useState(DEMO_DATE);
  const [status, setStatus] = useState(
    initial.mode === "local"
      ? "已保存至浏览器"
      : initial.mode === "corrupt"
        ? "读取失败 · 原数据已保留"
        : "仅本次会话",
  );
  const [editorId, setEditorId] = useState<string | null>(null);
  const [editorError, setEditorError] = useState("");
  const [planSide, setPlanSide] = useState<"buy" | "sell">("buy");
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [hotEditor, setHotEditor] = useState<Hot | null>(null);
  const [hotError, setHotError] = useState("");
  const [deleteHot, setDeleteHot] = useState(false);
  const [buyEditor, setBuyEditor] = useState<Buy | null>(null);
  const [buyError, setBuyError] = useState("");
  const [showBuys, setShowBuys] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [methodEditor, setMethodEditor] = useState<{
    buy: string;
    sell: string;
  } | null>(null);
  const [showReading, setShowReading] = useState(false);
  const [readingNotes, setReadingNotes] = useState(true);
  const [position, setPosition] = useState({
    x: Math.max(12, (window.innerWidth - 900) / 2),
    y: 100,
  });
  const drag = useRef<{
    x: number;
    y: number;
    left: number;
    top: number;
  } | null>(null);
  const latest = useRef(data);
  latest.current = data;
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const statusCallback = useRef(onSaveStatus);
  statusCallback.current = onSaveStatus;
  const consumed = useRef(new Set<string>());
  const day = data.days[date] ?? blankDay();
  const draft = day.drafts.find((item) => item.id === editorId);
  const disabled = mode === "corrupt";
  const recentBuys = sortBuy(data.buys)
    .slice(0, 10)
    .sort((a, b) => b.profit - a.profit);
  const historyDates = Object.keys(data.days).sort().reverse();
  const highlight = (sector: string) =>
    data.buys.some(
      (buy) => buy.profit > 10 && buy.sector.trim() === sector.trim(),
    );

  function flush() {
    if (modeRef.current === "corrupt") return;
    if (modeRef.current === "session") {
      setStatus("仅本次会话");
      return;
    }
    try {
      localStorage.setItem(KEY, JSON.stringify(latest.current));
      setStatus("已保存至浏览器");
    } catch {
      setStatus("保存失败 · 内容保留在本次会话");
    }
  }

  useEffect(() => {
    if (mode === "corrupt") return;
    if (mode === "session") {
      setStatus("仅本次会话");
      return;
    }
    setStatus("保存中…");
    const timeout = window.setTimeout(flush, 700);
    return () => window.clearTimeout(timeout);
  }, [data, mode]);
  useEffect(() => {
    statusCallback.current?.(status);
  }, [status]);
  useEffect(() => {
    const save = () => flush();
    window.addEventListener("pagehide", save);
    return () => {
      window.removeEventListener("pagehide", save);
      save();
    };
  }, []);
  useEffect(() => {
    if (
      !incoming ||
      consumed.current.has(incoming.requestId) ||
      mode === "corrupt"
    )
      return;
    consumed.current.add(incoming.requestId);
    const targetDate = dateValid(incoming.date) ? incoming.date : DEMO_DATE;
    const id = newId();
    setData((current) => {
      const target = current.days[targetDate] ?? blankDay();
      if (target.drafts.some((item) => item.requestId === incoming.requestId))
        return current;
      const nextDraft = {
        ...blankPlan(),
        id,
        sector: incoming.sector,
        basis: incoming.reason ?? "",
        codes: incoming.codes,
        source: `${pageNames[incoming.origin]}引用 · ${dateLabel(targetDate)}`,
        requestId: incoming.requestId,
      };
      return {
        ...current,
        days: {
          ...current.days,
          [targetDate]: { ...target, drafts: [...target.drafts, nextDraft] },
        },
      };
    });
    setDate(targetDate);
    setEditorId(id);
    setEditorError("");
  }, [incoming, mode]);

  function updateDay(updater: (previous: Day) => Day, targetDate = date) {
    setData((current) => ({
      ...current,
      days: {
        ...current.days,
        [targetDate]: updater(current.days[targetDate] ?? blankDay()),
      },
    }));
  }
  function chooseDate(value: string) {
    if (!dateValid(value)) return;
    flush();
    setDate(value);
    setEditorId(null);
    setEditorError("");
  }
  function changeDate(delta: number) {
    const next = new Date(`${date}T12:00:00Z`);
    next.setUTCDate(next.getUTCDate() + delta);
    chooseDate(next.toISOString().slice(0, 10));
  }
  function startDraft(plan?: Plan) {
    const existing =
      plan && day.drafts.find((item) => item.editingId === plan.id);
    const next =
      existing ??
      (plan ? { ...plan, id: newId(), editingId: plan.id } : blankPlan());
    if (!existing)
      updateDay((value) => ({ ...value, drafts: [...value.drafts, next] }));
    setEditorId(next.id);
    setEditorError("");
    setPlanSide(plan && !plan.method && plan.sellMethod ? "sell" : "buy");
  }
  function editDraft(patch: Partial<Draft>) {
    setEditorError("");
    updateDay((value) => ({
      ...value,
      drafts: value.drafts.map((item) =>
        item.id === editorId ? { ...item, ...patch } : item,
      ),
    }));
  }
  function submitDraft() {
    if (!draft) return;
    const buyEmpty = emptySide([draft.method, draft.entry, draft.avoid]);
    const sellEmpty = emptySide([draft.sellMethod, draft.exit, draft.hold]);
    let message = "";
    if (!draft.sector.trim())
      message = "请填写板块名称。未完成的内容已保留为草稿。";
    else if (buyEmpty && sellEmpty)
      message = "至少制定一侧计划：填写手法和触发条件。";
    else if (!buyEmpty && (!draft.method.trim() || !draft.entry.trim()))
      message =
        "买入计划需同时填写买点手法和买入条件；仅填写放弃条件不能追加。";
    else if (!sellEmpty && (!draft.sellMethod.trim() || !draft.exit.trim()))
      message = "卖出计划需同时填写卖点手法和卖出条件。";
    if (message) {
      setEditorError(message);
      return;
    }
    const { editingId, requestId: _requestId, ...plan } = draft;
    const result = {
      ...plan,
      id: editingId ?? draft.id,
      sector: draft.sector.trim(),
    };
    updateDay((value) => ({
      ...value,
      plans: editingId
        ? value.plans.map((item) => (item.id === editingId ? result : item))
        : [...value.plans, result],
      drafts: value.drafts.filter((item) => item.id !== draft.id),
    }));
    setEditorId(null);
    onNotice(
      editingId ? "已更新这条预案" : "已加入当日预案，条件由你自行判断与执行",
    );
  }
  function closeReading() {
    flush();
    setShowReading(false);
  }
  function openReading() {
    setPosition({
      x: Math.max(
        12,
        (window.innerWidth - Math.min(900, window.innerWidth - 24)) / 2,
      ),
      y: Math.min(100, Math.max(12, (window.innerHeight - 650) / 2)),
    });
    setShowReading(true);
  }
  function requestHotClose() {
    if (!hotEditor) return;
    const original = data.hotspots.find((hot) => hot.id === hotEditor.id);
    const changed = original
      ? JSON.stringify(original) !== JSON.stringify(hotEditor)
      : !!(hotEditor.sector.trim() || hotEditor.heights.trim());
    if (!changed || window.confirm("放弃尚未保存的热点修改？")) {
      setHotEditor(null);
      setDeleteHot(false);
    }
  }
  function requestBuyClose() {
    if (!buyEditor) return;
    const original = data.buys.find((buy) => buy.id === buyEditor.id);
    const changed = original
      ? JSON.stringify(original) !== JSON.stringify(buyEditor)
      : !!(
          buyEditor.stock ||
          buyEditor.sector ||
          buyEditor.note ||
          buyEditor.profit
        );
    if (!changed || window.confirm("放弃尚未保存的买入记录修改？"))
      setBuyEditor(null);
  }
  function renderPlan(plan: Plan, index: number, reading = false) {
    return (
      <article
        className="journal-plan"
        data-testid="journal-plan"
        key={plan.id}
      >
        <header className="journal-plan-heading">
          <div className="journal-plan-name">
            <span className="journal-plan-number">
              {String(index + 1).padStart(2, "0")}
            </span>
            <h3>{plan.sector}</h3>
            <span className="journal-source-tag">{plan.source}</span>
          </div>
          {!reading && (
            <div className="journal-plan-actions">
              <button
                className="journal-icon-button"
                aria-label={`编辑${plan.sector}预案`}
                disabled={disabled}
                onClick={() => startDraft(plan)}
              >
                <Pencil size={14} />
              </button>
              <button
                className="journal-icon-button"
                aria-label={`删除${plan.sector}预案`}
                disabled={disabled}
                onClick={() => setDeleteId(plan.id)}
              >
                <Trash2 size={14} />
              </button>
            </div>
          )}
        </header>
        {plan.codes.length > 0 && (
          <div className="journal-plan-stocks">
            {plan.codes.map((code) => (
              <button
                key={code}
                onClick={() =>
                  onInspect(code, {
                    date,
                    sector: plan.sector,
                    codes: [code],
                    reason: plan.basis,
                    origin: "journal",
                  })
                }
              >
                {stocks.find((stock) => stock.code === code)?.name ?? code}
                <span>{code}</span>
                <ArrowUpRight size={11} />
              </button>
            ))}
            {!reading && (
              <button
                className="journal-watch-link"
                onClick={() =>
                  onAddToWatch({
                    date,
                    sector: plan.sector,
                    codes: plan.codes,
                    origin: "journal",
                    reason: plan.basis,
                  })
                }
              >
                加入盯盘 <Plus size={12} />
              </button>
            )}
          </div>
        )}
        {plan.basis && (
          <p className="journal-plan-basis">
            <span>判断依据</span>
            {plan.basis}
          </p>
        )}
        <div className="journal-plan-sides">
          <section>
            <div className="journal-side-heading">
              <ArrowUpRight size={14} />
              <b>买入计划</b>
              {plan.method && <span>{plan.method}</span>}
            </div>
            {plan.method || plan.entry ? (
              <>
                <p>
                  <span>触发</span>
                  {plan.entry}
                </p>
                <p className="journal-secondary-condition">
                  <span>放弃</span>
                  {plan.avoid || "未制定"}
                </p>
              </>
            ) : (
              <div className="journal-not-planned">
                未制定 <span>保留观察，等待条件</span>
              </div>
            )}
          </section>
          <section>
            <div className="journal-side-heading sell">
              <ArrowDownLeft size={14} />
              <b>卖出计划</b>
              {plan.sellMethod && <span>{plan.sellMethod}</span>}
            </div>
            {plan.sellMethod || plan.exit ? (
              <>
                <p>
                  <span>触发</span>
                  {plan.exit}
                </p>
                <p className="journal-secondary-condition">
                  <span>持仓</span>
                  {plan.hold || "未制定"}
                </p>
              </>
            ) : (
              <div className="journal-not-planned">
                未制定 <span>尚无卖出安排</span>
              </div>
            )}
          </section>
        </div>
      </article>
    );
  }

  return (
    <div className="u-page journal-page" data-testid="journal-page">
      <div className="u-page-heading journal-page-heading">
        <div>
          <div className="u-eyebrow">REVIEW & PLAN</div>
          <h1 className="u-page-title">
            复盘 <span>把判断写在交易之前</span>
          </h1>
        </div>
        <div className="journal-heading-tools">
          <div className="journal-date-control">
            <button aria-label="上一日复盘" onClick={() => changeDate(-1)}>
              <ChevronLeft size={15} />
            </button>
            <CalendarDays size={15} />
            <input
              aria-label="复盘日期"
              type="date"
              value={date}
              onChange={(event) => chooseDate(event.target.value)}
            />
            <button aria-label="下一日复盘" onClick={() => changeDate(1)}>
              <ChevronRight size={15} />
            </button>
          </div>
          <button
            className="u-button"
            aria-label="打开复盘阅读浮窗"
            onClick={openReading}
          >
            <Maximize2 size={15} />
            阅读浮窗
          </button>
          <button
            className="u-button primary"
            disabled={disabled}
            onClick={() => startDraft()}
          >
            <Plus size={16} />
            新建预案
          </button>
        </div>
      </div>
      {mode !== "local" || status.startsWith("保存失败") ? (
        <div className="journal-storage-notice" role="status">
          {mode === "corrupt"
            ? "浏览器中的复盘数据读取失败，原内容已保留。"
            : mode === "session"
              ? "浏览器存储不可用，修改仅保留在本次会话。"
              : "无法写入浏览器存储，当前内容仍保留在内存中。"}
          {mode === "corrupt" ? (
            <>
              <button
                onClick={() => {
                  const retry = loadJournal();
                  if (retry.mode === "local") {
                    setData(retry.data);
                    setMode("local");
                  } else onNotice("仍无法读取，请保留原浏览器数据后再检查。");
                }}
              >
                重试读取
              </button>
              <button onClick={() => setMode("session")}>仅本次会话继续</button>
            </>
          ) : mode === "local" ? (
            <button onClick={flush}>重试保存</button>
          ) : null}
        </div>
      ) : null}
      <section className="journal-hotspots">
        <div className="journal-hot-label">
          <span className="journal-hot-dot" />
          <strong>当前热点</strong>
          <span>人工记录 · 最多 5 条</span>
        </div>
        <div className="journal-hot-chips">
          {sortHot(data.hotspots).map((hot) => (
            <button
              key={hot.id}
              className={`journal-hot-chip ${highlight(hot.sector) ? "highlight" : ""}`}
              onClick={() => {
                setHotEditor({ ...hot });
                setHotError("");
                setDeleteHot(false);
              }}
            >
              <span>{dateLabel(hot.date)}</span>
              <b>{hot.sector}</b>
              <ChevronRight size={12} />
            </button>
          ))}
          <button
            className="journal-hot-add"
            aria-label="新增人工热点"
            disabled={disabled}
            onClick={() => {
              setHotEditor({
                id: newId(),
                date,
                sector: "",
                heights: "",
                created: Date.now(),
              });
              setHotError("");
              setDeleteHot(false);
            }}
          >
            <Plus size={16} />
          </button>
        </div>
      </section>
      <div className="journal-body">
        <main className="journal-main">
          <div className="journal-section-label">
            <div>
              <NotebookPen size={17} />
              <h2>当日预案</h2>
              <span className="journal-count">{day.plans.length}</span>
              <span className="journal-muted">{date} · 演示文档</span>
            </div>
            <div className="journal-save-state" aria-live="polite">
              <span className={status.startsWith("保存失败") ? "error" : ""}>
                {status === "已保存至浏览器" && <Check size={12} />}
                {status}
              </span>
              <button
                aria-label="立即保存复盘"
                disabled={disabled}
                onClick={flush}
              >
                <Save size={14} />
              </button>
            </div>
          </div>
          {day.drafts.length > 0 && (
            <div className="journal-drafts">
              <FilePenLine size={14} />
              <span>{day.drafts.length} 份草稿待完成</span>
              {day.drafts.map((item) => (
                <button
                  key={item.id}
                  onClick={() => {
                    setEditorId(item.id);
                    setEditorError("");
                  }}
                >
                  {item.sector || "未命名预案"}
                  <ChevronRight size={12} />
                </button>
              ))}
            </div>
          )}
          <div className="journal-plans-scroll">
            {day.plans.length ? (
              day.plans.map((plan, index) => renderPlan(plan, index))
            ) : (
              <div className="journal-empty">
                <BookOpen size={32} />
                <h3>给这一天写一份清晰的预案</h3>
                <p>从买入或卖出任一侧开始；未完成内容会保留为草稿。</p>
                <button
                  className="u-button primary"
                  disabled={disabled}
                  onClick={() => startDraft()}
                >
                  <Plus size={15} />
                  新建预案
                </button>
              </div>
            )}
          </div>
          <section className="journal-note-panel">
            <header>
              <div>
                <FilePenLine size={15} />
                <h2>盘中记录</h2>
                <span>记录观察，也记录没有执行的原因</span>
              </div>
              <span className="journal-note-count">{day.note.length} 字</span>
            </header>
            <textarea
              aria-label="盘中记录"
              disabled={disabled}
              value={day.note}
              placeholder="写下时间、观察与判断；修改会自动保存…"
              onChange={(event) => {
                const note = event.target.value;
                updateDay((value) => ({ ...value, note }));
              }}
            />
          </section>
        </main>
        <aside className="journal-sidebar">
          <section className="journal-history-panel u-panel">
            <header className="u-panel-heading">
              <div>
                <History size={16} />
                <h2>历史复盘</h2>
              </div>
              <button onClick={() => setShowHistory(true)}>
                全部历史 <ChevronRight size={13} />
              </button>
            </header>
            <div className="journal-history-list">
              {historyDates.slice(0, 10).map((item, index) => (
                <button
                  className={`journal-history-item ${item === date ? "selected" : ""}`}
                  key={item}
                  onClick={() => chooseDate(item)}
                >
                  <span className="journal-history-line">
                    <span>{dateLabel(item)}</span>
                    <i />
                  </span>
                  <span>
                    <b>
                      {item === DEMO_DATE
                        ? "今日预案"
                        : item === "2026-10-13"
                          ? "分歧中的确认"
                          : "观察与等待"}
                    </b>
                    <small>
                      {data.days[item].plans
                        .map((plan) => plan.sector)
                        .join(" · ") || "尚无完整预案"}
                    </small>
                  </span>
                  <em>{data.days[item].plans.length} 份</em>
                  {index === 0 && (
                    <span className="journal-history-latest">最新</span>
                  )}
                </button>
              ))}
            </div>
            <div className="journal-history-footer">
              保留各日预案、草稿与盘中记录
            </div>
          </section>
          <section className="journal-buys-panel u-panel">
            <header className="u-panel-heading">
              <div>
                <Clock3 size={16} />
                <h2>最近 10 笔买入</h2>
              </div>
              <button onClick={() => setShowBuys(true)}>
                全部历史 <ChevronRight size={13} />
              </button>
            </header>
            <p className="journal-buys-caption">最近 10 笔，再按手工收益排序</p>
            <div className="journal-buy-list">
              <div className="journal-buy-columns">
                <span>股票 / 买入日</span>
                <span>手工收益</span>
              </div>
              {recentBuys.map((buy) => (
                <button
                  key={buy.id}
                  className={`journal-buy-row ${buy.profit > 10 ? "highlight" : ""}`}
                  onClick={() => {
                    setBuyEditor({ ...buy });
                    setBuyError("");
                  }}
                >
                  <span>
                    <b>{buy.stock}</b>
                    <small>
                      {dateLabel(buy.date)}
                      <i />
                      {buy.sector}
                    </small>
                  </span>
                  <strong className={buy.profit >= 0 ? "positive" : "negative"}>
                    {buy.profit > 0 ? "+" : ""}
                    {buy.profit.toFixed(2)}
                    <small>%</small>
                  </strong>
                </button>
              ))}
            </div>
            <div className="journal-buy-footer">
              <span>人工填写 · 非账户收益</span>
              <button
                disabled={disabled}
                onClick={() => {
                  setBuyEditor({
                    id: newId(),
                    date,
                    stock: "",
                    sector: "",
                    method: "",
                    profit: 0,
                    note: "",
                  });
                  setBuyError("");
                }}
              >
                <Plus size={13} />
                补录
              </button>
            </div>
          </section>
        </aside>
      </div>
      {draft && (
        <JournalDialog
          title={draft.editingId ? "编辑预案" : "编写预案"}
          className="journal-editor"
          onClose={() => {
            flush();
            setEditorId(null);
          }}
        >
          <div data-testid="journal-editor">
            <div className="journal-editor-context">
              <span>{date}</span>
              <span>{draft.source}</span>
              <span>草稿自动保存</span>
            </div>
            <div className="journal-editor-fields">
              <label>
                板块名称 <span>必填</span>
                <input
                  aria-label="板块名称"
                  value={draft.sector}
                  maxLength={40}
                  placeholder="例如：文化传媒"
                  onChange={(event) =>
                    editDraft({ sector: event.target.value })
                  }
                />
              </label>
              <label>
                赚钱效应与判断依据
                <textarea
                  aria-label="赚钱效应与判断依据"
                  value={draft.basis}
                  rows={2}
                  placeholder="写下判断依据、信息来源与需要验证的观察…"
                  onChange={(event) => editDraft({ basis: event.target.value })}
                />
              </label>
              <div className="journal-editor-stocks">
                <span>关联股票</span>
                {draft.codes.map((code) => (
                  <button
                    key={code}
                    className="journal-stock-chip"
                    onClick={() =>
                      editDraft({
                        codes: draft.codes.filter((item) => item !== code),
                      })
                    }
                  >
                    {stocks.find((stock) => stock.code === code)?.name ?? code}
                    <X size={11} />
                  </button>
                ))}
                <select
                  aria-label="添加预案关联股票"
                  value=""
                  onChange={(event) => {
                    if (
                      event.target.value &&
                      !draft.codes.includes(event.target.value)
                    )
                      editDraft({
                        codes: [...draft.codes, event.target.value],
                      });
                  }}
                >
                  <option value="">＋ 添加股票</option>
                  {stocks
                    .filter((stock) => !draft.codes.includes(stock.code))
                    .map((stock) => (
                      <option key={stock.code} value={stock.code}>
                        {stock.name} {stock.code}
                      </option>
                    ))}
                </select>
              </div>
              <div className="journal-editor-tabs">
                <div className="u-segmented">
                  <button
                    className={planSide === "buy" ? "active" : ""}
                    onClick={() => setPlanSide("buy")}
                  >
                    买入计划{draft.method && draft.entry && <Check size={12} />}
                  </button>
                  <button
                    className={planSide === "sell" ? "active" : ""}
                    onClick={() => setPlanSide("sell")}
                  >
                    卖出计划
                    {draft.sellMethod && draft.exit && <Check size={12} />}
                  </button>
                </div>
                <button
                  className="journal-text-button"
                  onClick={() =>
                    setMethodEditor({
                      buy: data.methods.join("\n"),
                      sell: data.sellMethods.join("\n"),
                    })
                  }
                >
                  管理手法
                </button>
              </div>
              {planSide === "buy" ? (
                <>
                  <label>
                    买点手法
                    <input
                      aria-label="买点手法"
                      list="journal-buy-methods"
                      value={draft.method}
                      placeholder="手工填写，或使用已保存手法"
                      onChange={(event) =>
                        editDraft({ method: event.target.value })
                      }
                    />
                  </label>
                  <label>
                    买入条件
                    <textarea
                      aria-label="买入条件"
                      rows={3}
                      value={draft.entry}
                      placeholder="描述可以观察和验证的触发条件…"
                      onChange={(event) =>
                        editDraft({ entry: event.target.value })
                      }
                    />
                  </label>
                  <label>
                    放弃买入条件 <span>选填</span>
                    <textarea
                      aria-label="放弃买入条件"
                      rows={2}
                      value={draft.avoid}
                      placeholder="什么情况出现时，放弃这笔机会？"
                      onChange={(event) =>
                        editDraft({ avoid: event.target.value })
                      }
                    />
                  </label>
                </>
              ) : (
                <>
                  <label>
                    卖点手法
                    <input
                      aria-label="卖点手法"
                      list="journal-sell-methods"
                      value={draft.sellMethod}
                      placeholder="可独立制定卖出预案，无需先有买入记录"
                      onChange={(event) =>
                        editDraft({ sellMethod: event.target.value })
                      }
                    />
                  </label>
                  <label>
                    卖出条件
                    <textarea
                      aria-label="卖出条件"
                      rows={3}
                      value={draft.exit}
                      placeholder="写下减持或退出需要满足的条件…"
                      onChange={(event) =>
                        editDraft({ exit: event.target.value })
                      }
                    />
                  </label>
                  <label>
                    继续持仓条件 <span>选填</span>
                    <textarea
                      aria-label="继续持仓条件"
                      rows={2}
                      value={draft.hold}
                      placeholder="什么情况下仍然保持原计划？"
                      onChange={(event) =>
                        editDraft({ hold: event.target.value })
                      }
                    />
                  </label>
                </>
              )}
              <datalist id="journal-buy-methods">
                {data.methods.map((method) => (
                  <option key={method} value={method} />
                ))}
              </datalist>
              <datalist id="journal-sell-methods">
                {data.sellMethods.map((method) => (
                  <option key={method} value={method} />
                ))}
              </datalist>
              {editorError && (
                <p className="journal-error" role="alert">
                  {editorError}
                </p>
              )}
            </div>
            <footer className="journal-modal-footer">
              <span>允许仅制定一侧计划</span>
              <button
                className="u-button"
                onClick={() => {
                  flush();
                  setEditorId(null);
                }}
              >
                保留草稿
              </button>
              <button className="u-button primary" onClick={submitDraft}>
                <Check size={15} />
                {draft.editingId ? "更新预案" : "加入当日预案"}
              </button>
            </footer>
          </div>
        </JournalDialog>
      )}
      {deleteId && (
        <JournalDialog title="删除这条预案？" onClose={() => setDeleteId(null)}>
          <div className="journal-dialog-body">
            <p>
              将从 {date} 的结果中移除“
              {day.plans.find((plan) => plan.id === deleteId)?.sector}
              ”预案及其编辑草稿。盘中记录和其他预案会保留。
            </p>
          </div>
          <footer className="journal-modal-footer">
            <button className="u-button" onClick={() => setDeleteId(null)}>
              取消
            </button>
            <button
              className="u-button journal-danger"
              onClick={() => {
                updateDay((value) => ({
                  ...value,
                  plans: value.plans.filter((plan) => plan.id !== deleteId),
                  drafts: value.drafts.filter(
                    (item) => item.editingId !== deleteId,
                  ),
                }));
                setDeleteId(null);
                onNotice("已删除这条预案");
              }}
            >
              删除预案
            </button>
          </footer>
        </JournalDialog>
      )}
      {hotEditor && (
        <JournalDialog
          title={
            data.hotspots.some((hot) => hot.id === hotEditor.id)
              ? "人工热点详情"
              : "新增人工热点"
          }
          onClose={requestHotClose}
        >
          <div className="journal-dialog-body journal-form">
            <p className="journal-help">
              仅保留最新 5 条。由你手工记录，与自动题材分析独立。
            </p>
            <label>
              日期
              <input
                type="date"
                value={hotEditor.date}
                onChange={(event) =>
                  setHotEditor({ ...hotEditor, date: event.target.value })
                }
              />
            </label>
            <label>
              板块名称
              <input
                value={hotEditor.sector}
                maxLength={40}
                onChange={(event) =>
                  setHotEditor({ ...hotEditor, sector: event.target.value })
                }
              />
            </label>
            <label>
              高度股票与观察
              <textarea
                aria-label="高度股票与观察"
                rows={4}
                value={hotEditor.heights}
                placeholder="手工填写股票名称、连板高度或观察记录"
                onChange={(event) =>
                  setHotEditor({ ...hotEditor, heights: event.target.value })
                }
              />
            </label>
            {hotError && (
              <p className="journal-error" role="alert">
                {hotError}
              </p>
            )}
            {deleteHot && (
              <p className="journal-error">
                删除后该条热点不再保留，也不会进入历史归档。
              </p>
            )}
          </div>
          <footer className="journal-modal-footer">
            {data.hotspots.some((hot) => hot.id === hotEditor.id) && (
              <button
                className="journal-text-button journal-delete"
                disabled={disabled}
                onClick={() => {
                  if (!deleteHot) {
                    setDeleteHot(true);
                    return;
                  }
                  setData((current) => ({
                    ...current,
                    hotspots: current.hotspots.filter(
                      (hot) => hot.id !== hotEditor.id,
                    ),
                  }));
                  setHotEditor(null);
                }}
              >
                {" "}
                {deleteHot ? "确认删除热点" : "删除热点"}
              </button>
            )}
            <button className="u-button" onClick={requestHotClose}>
              取消
            </button>
            <button
              className="u-button primary"
              disabled={disabled}
              onClick={() => {
                if (!dateValid(hotEditor.date) || !hotEditor.sector.trim()) {
                  setHotError("请填写有效日期和板块名称。");
                  return;
                }
                const next = { ...hotEditor, sector: hotEditor.sector.trim() };
                const ordered = sortHot([
                  ...data.hotspots.filter((hot) => hot.id !== next.id),
                  next,
                ]);
                if (!ordered.slice(0, 5).some((hot) => hot.id === next.id)) {
                  setHotError(
                    "这个日期早于已保留的 5 条热点，不能挤掉更新记录。",
                  );
                  return;
                }
                setData((current) => ({
                  ...current,
                  hotspots: ordered.slice(0, 5),
                }));
                setHotEditor(null);
                onNotice("人工热点已保存");
              }}
            >
              保存热点
            </button>
          </footer>
        </JournalDialog>
      )}
      {showBuys && (
        <JournalDialog
          title="全部买入历史"
          className="journal-wide-dialog"
          onClose={() => setShowBuys(false)}
        >
          <div className="journal-dialog-body">
            <p className="journal-help">
              按买入日期降序排列 ·
              收益率由人工填写，不推导卖出日期、仓位或盈亏金额。
            </p>
            <table className="journal-history-table">
              <thead>
                <tr>
                  <th>买入日期</th>
                  <th>股票</th>
                  <th>板块</th>
                  <th>买点手法</th>
                  <th>手工收益</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {sortBuy(data.buys).map((buy) => (
                  <tr
                    key={buy.id}
                    className={buy.profit > 10 ? "highlight" : ""}
                  >
                    <td>{buy.date}</td>
                    <td>{buy.stock}</td>
                    <td>{buy.sector}</td>
                    <td>{buy.method || "—"}</td>
                    <td className={buy.profit >= 0 ? "positive" : "negative"}>
                      {buy.profit > 0 ? "+" : ""}
                      {buy.profit.toFixed(2)}%
                    </td>
                    <td>
                      <button
                        aria-label={`编辑${buy.stock}买入记录`}
                        onClick={() => {
                          setBuyEditor({ ...buy });
                          setBuyError("");
                        }}
                      >
                        <Pencil size={13} />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <footer className="journal-modal-footer">
            <span>{data.buys.length} 笔演示记录</span>
            <button
              className="u-button primary"
              disabled={disabled}
              onClick={() => {
                setBuyEditor({
                  id: newId(),
                  date,
                  stock: "",
                  sector: "",
                  method: "",
                  profit: 0,
                  note: "",
                });
                setBuyError("");
              }}
            >
              <Plus size={14} />
              补录买入
            </button>
          </footer>
        </JournalDialog>
      )}
      {buyEditor && (
        <JournalDialog title="手工买入记录" onClose={requestBuyClose}>
          <div className="journal-dialog-body journal-form">
            <p className="journal-help">
              收益率仅作为手工复盘记录。严格超过 10% 的记录会浅红高亮。
            </p>
            <div className="journal-form-columns">
              <label>
                买入日期
                <input
                  type="date"
                  value={buyEditor.date}
                  onChange={(event) =>
                    setBuyEditor({ ...buyEditor, date: event.target.value })
                  }
                />
              </label>
              <label>
                股票文本
                <input
                  value={buyEditor.stock}
                  onChange={(event) =>
                    setBuyEditor({ ...buyEditor, stock: event.target.value })
                  }
                />
              </label>
            </div>
            <label>
              板块
              <input
                value={buyEditor.sector}
                onChange={(event) =>
                  setBuyEditor({ ...buyEditor, sector: event.target.value })
                }
              />
            </label>
            <div className="journal-form-columns">
              <label>
                买点手法
                <input
                  value={buyEditor.method}
                  list="journal-history-buy-methods"
                  onChange={(event) =>
                    setBuyEditor({ ...buyEditor, method: event.target.value })
                  }
                />
                <datalist id="journal-history-buy-methods">
                  {data.methods.map((method) => (
                    <option key={method} value={method} />
                  ))}
                </datalist>
              </label>
              <label>
                手工收益率（%）
                <input
                  type="number"
                  step="0.01"
                  value={Number.isNaN(buyEditor.profit) ? "" : buyEditor.profit}
                  onChange={(event) =>
                    setBuyEditor({
                      ...buyEditor,
                      profit:
                        event.target.value === ""
                          ? Number.NaN
                          : Number(event.target.value),
                    })
                  }
                />
              </label>
            </div>
            <label>
              备注
              <textarea
                aria-label="买入记录备注"
                rows={3}
                value={buyEditor.note}
                onChange={(event) =>
                  setBuyEditor({ ...buyEditor, note: event.target.value })
                }
              />
            </label>
            {buyError && (
              <p className="journal-error" role="alert">
                {buyError}
              </p>
            )}
          </div>
          <footer className="journal-modal-footer">
            <button className="u-button" onClick={requestBuyClose}>
              取消
            </button>
            <button
              className="u-button primary"
              disabled={disabled}
              onClick={() => {
                if (
                  !dateValid(buyEditor.date) ||
                  !buyEditor.stock.trim() ||
                  !buyEditor.sector.trim() ||
                  !Number.isFinite(buyEditor.profit)
                ) {
                  setBuyError("请填写有效日期、股票、板块和收益率。");
                  return;
                }
                const next = {
                  ...buyEditor,
                  stock: buyEditor.stock.trim(),
                  sector: buyEditor.sector.trim(),
                };
                setData((current) => ({
                  ...current,
                  buys: current.buys.some((buy) => buy.id === next.id)
                    ? current.buys.map((buy) =>
                        buy.id === next.id ? next : buy,
                      )
                    : [...current.buys, next],
                }));
                setBuyEditor(null);
                onNotice("买入记录已保存，收益高亮已重新计算");
              }}
            >
              保存记录
            </button>
          </footer>
        </JournalDialog>
      )}
      {showHistory && (
        <JournalDialog title="复盘历史" onClose={() => setShowHistory(false)}>
          <div className="journal-dialog-body journal-all-history">
            {historyDates.map((item) => (
              <button
                key={item}
                onClick={() => {
                  chooseDate(item);
                  setShowHistory(false);
                }}
              >
                <CalendarDays size={18} />
                <span>
                  <strong>{item}</strong>
                  <small>
                    {data.days[item].plans
                      .map((plan) => plan.sector)
                      .join(" · ") || "尚无完整预案"}
                  </small>
                </span>
                <span>
                  {data.days[item].plans.length} 份预案 ·{" "}
                  {data.days[item].drafts.length} 份草稿
                </span>
                <ChevronRight size={15} />
              </button>
            ))}
          </div>
        </JournalDialog>
      )}
      {methodEditor && (
        <JournalDialog
          title="管理预案手法"
          onClose={() => {
            if (
              (methodEditor.buy !== data.methods.join("\n") ||
                methodEditor.sell !== data.sellMethods.join("\n")) &&
              !window.confirm("放弃尚未保存的手法修改？")
            )
              return;
            setMethodEditor(null);
          }}
        >
          <div className="journal-dialog-body journal-form">
            <p className="journal-help">
              每行一个手法，保存时去除首尾空白和重复项。删除选项不会修改历史文本。
            </p>
            <div className="journal-form-columns">
              <label>
                买点手法
                <textarea
                  aria-label="买点手法选项"
                  rows={7}
                  value={methodEditor.buy}
                  placeholder="填写你的买点手法"
                  onChange={(event) =>
                    setMethodEditor({
                      ...methodEditor,
                      buy: event.target.value,
                    })
                  }
                />
              </label>
              <label>
                卖点手法
                <textarea
                  aria-label="卖点手法选项"
                  rows={7}
                  value={methodEditor.sell}
                  placeholder="填写你的卖点手法"
                  onChange={(event) =>
                    setMethodEditor({
                      ...methodEditor,
                      sell: event.target.value,
                    })
                  }
                />
              </label>
            </div>
          </div>
          <footer className="journal-modal-footer">
            <button
              className="u-button"
              onClick={() => {
                if (
                  (methodEditor.buy !== data.methods.join("\n") ||
                    methodEditor.sell !== data.sellMethods.join("\n")) &&
                  !window.confirm("放弃尚未保存的手法修改？")
                )
                  return;
                setMethodEditor(null);
              }}
            >
              取消
            </button>
            <button
              className="u-button primary"
              onClick={() => {
                const parse = (text: string) => [
                  ...new Set(
                    text
                      .split("\n")
                      .map((line) => line.trim())
                      .filter(Boolean),
                  ),
                ];
                setData((current) => ({
                  ...current,
                  methods: parse(methodEditor.buy),
                  sellMethods: parse(methodEditor.sell),
                }));
                setMethodEditor(null);
              }}
            >
              保存手法
            </button>
          </footer>
        </JournalDialog>
      )}
      {showReading && (
        <div
          className="journal-reading-window"
          data-testid="journal-reading-window"
          role="dialog"
          aria-label="复盘阅读浮窗"
          style={{ left: position.x, top: position.y }}
          onKeyDown={(event) => {
            if (
              event.key === "Escape" &&
              !event.nativeEvent.isComposing &&
              event.keyCode !== 229
            ) {
              event.stopPropagation();
              closeReading();
            }
          }}
        >
          <header
            className="journal-reading-title"
            onPointerDown={(event) => {
              if ((event.target as HTMLElement).closest("button")) return;
              drag.current = {
                x: event.clientX,
                y: event.clientY,
                left: position.x,
                top: position.y,
              };
              event.currentTarget.setPointerCapture(event.pointerId);
            }}
            onPointerMove={(event) => {
              if (!drag.current) return;
              setPosition({
                x: Math.max(
                  0,
                  Math.min(
                    Math.max(
                      0,
                      window.innerWidth -
                        event.currentTarget.getBoundingClientRect().width,
                    ),
                    drag.current.left + event.clientX - drag.current.x,
                  ),
                ),
                y: Math.max(
                  0,
                  Math.min(
                    window.innerHeight - 80,
                    drag.current.top + event.clientY - drag.current.y,
                  ),
                ),
              });
            }}
            onPointerUp={() => {
              drag.current = null;
            }}
            onPointerCancel={() => {
              drag.current = null;
            }}
          >
            <Grip size={16} />
            <strong>复盘 · 阅读模式</strong>
            <span>{date}</span>
            <span className="journal-window-demo">网页浮窗预览</span>
            <button
              className="journal-icon-button"
              aria-label="关闭复盘阅读浮窗"
              onClick={closeReading}
            >
              <X size={17} />
            </button>
          </header>
          <div className="journal-reading-toolbar">
            <span>
              <BookOpen size={15} />
              当日全部预案 <b>{day.plans.length}</b>
            </span>
            <span>{status}</span>
          </div>
          <div className="journal-reading-content">
            <div className="journal-reading-hotspots">
              <span>人工热点</span>
              {sortHot(data.hotspots).map((hot) => (
                <span
                  className={`journal-reading-chip ${highlight(hot.sector) ? "highlight" : ""}`}
                  key={hot.id}
                >
                  {hot.sector}
                </span>
              ))}
            </div>
            {day.plans.map((plan, index) => renderPlan(plan, index, true))}
            {!day.plans.length && (
              <div className="journal-empty">
                这一天还没有完整预案，草稿保留在主页面。
              </div>
            )}
            <section className="journal-reading-notes">
              <button onClick={() => setReadingNotes(!readingNotes)}>
                <ChevronDown
                  size={15}
                  style={{
                    transform: readingNotes ? "none" : "rotate(-90deg)",
                  }}
                />
                盘中记录<span>与主页面共享内容</span>
              </button>
              {readingNotes && (
                <textarea
                  aria-label="浮窗盘中记录"
                  value={day.note}
                  disabled={disabled}
                  onChange={(event) => {
                    const note = event.target.value;
                    updateDay((value) => ({ ...value, note }));
                  }}
                  placeholder="记录盘中观察…"
                />
              )}
            </section>
          </div>
          <footer className="journal-reading-footer">
            <span>仅作人工计划记录 · 不自动执行</span>
            <span>拖动标题栏移动 · 右下角调整大小</span>
          </footer>
        </div>
      )}
    </div>
  );
}
