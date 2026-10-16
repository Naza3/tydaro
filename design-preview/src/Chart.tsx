import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import "./Chart.css";

export type ChartProps = {
  symbol: { code: string; name: string; price: number; change: number };
  mode: "intraday" | "daily";
  onInspectDay?: (date: string) => void;
  historical?: boolean;
};

type Point = {
  time: string;
  minute: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  turnover: number;
  average: number;
  previous: number;
};
type Inspection = { index: number; y: number };
const BLUE = "#397af6";
const UP = "#ef4770";
const DOWN = "#009b9a";
const GOLD = "#d99935";
const MA_COLORS = [GOLD, "#7984cd", "#64a6a0"];
const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));
const fixed = (value: number) => value.toFixed(2);
const signed = (value: number) => `${value > 0 ? "+" : ""}${fixed(value)}`;
const hash = (text: string) =>
  [...text].reduce(
    (value, char) => (value * 31 + char.charCodeAt(0)) >>> 0,
    19,
  );
const noise = (n: number, seed: number) => {
  const value = Math.sin(n * 12.9898 + seed * 0.001) * 43758.5453;
  return value - Math.floor(value);
};
const timeLabel = (minute: number) =>
  `${String(Math.floor(minute / 60)).padStart(2, "0")}:${String(minute % 60).padStart(2, "0")}`;
const quantity = (value: number) =>
  value >= 1e8
    ? `${fixed(value / 1e8)} 亿`
    : value >= 1e4
      ? `${fixed(value / 1e4)} 万`
      : Math.round(value).toLocaleString("zh-CN");
const interpolate = (values: number[], progress: number) => {
  const index = clamp(progress, 0, 1) * (values.length - 1);
  const lower = Math.floor(index);
  return (
    values[lower] +
    (values[Math.min(lower + 1, values.length - 1)] - values[lower]) *
      (index - lower)
  );
};

function makeIntraday(
  symbol: ChartProps["symbol"],
  historical: boolean,
): Point[] {
  const seed = hash(symbol.code);
  const previous = symbol.price / (1 + symbol.change / 100);
  const minutes = [
    565,
    ...Array.from({ length: 121 }, (_, i) => 570 + i),
    ...Array.from({ length: historical ? 118 : 93 }, (_, i) => 780 + i),
    ...(historical ? [900] : []),
  ];
  let amount = 0;
  let volumeTotal = 0;
  let last = previous;
  return minutes.map((minute, index) => {
    const t = index / (minutes.length - 1);
    const trend = interpolate(
      [0.12, 0.38, 0.64, 0.57, 0.77, 0.72, 0.85, 0.94, 1],
      t,
    );
    const variation =
      (Math.sin(index * 0.56) * 0.0028 +
        Math.sin(index * 1.7) * 0.0019 +
        (noise(index, seed) - 0.5) * 0.003) *
      Math.sin(t * Math.PI);
    const close =
      index === minutes.length - 1
        ? symbol.price
        : previous + (symbol.price - previous) * trend + previous * variation;
    const volume = Math.round(
      (12000 + noise(index + 100, seed) * 65000) *
        (1 +
          Math.exp(-index / 16) * 4 +
          (index > 115 && index < 132 ? 0.9 : 0)),
    );
    const turnover = (volume * (last + close)) / 2;
    amount += turnover;
    volumeTotal += volume;
    const point = {
      time: timeLabel(minute),
      minute,
      open: last,
      high: Math.max(last, close),
      low: Math.min(last, close),
      close,
      volume,
      turnover,
      average: amount / volumeTotal,
      previous,
    };
    last = close;
    return point;
  });
}

function makeDaily(symbol: ChartProps["symbol"]): Point[] {
  const seed = hash(symbol.code);
  const dates: string[] = [];
  const cursor = new Date("2026-10-14T12:00:00Z");
  while (dates.length < 120) {
    if (cursor.getUTCDay() !== 0 && cursor.getUTCDay() !== 6)
      dates.unshift(cursor.toISOString().slice(0, 10));
    cursor.setUTCDate(cursor.getUTCDate() - 1);
  }
  let previous = symbol.price * 1.14;
  return dates.map((time, index) => {
    const t = index / (dates.length - 1);
    const trend = interpolate(
      [1.14, 1.03, 0.94, 0.78, 0.71, 0.67, 0.79, 0.84, 0.78, 0.88, 1],
      t,
    );
    const close =
      index === dates.length - 1
        ? symbol.price
        : index === dates.length - 2
          ? symbol.price / (1 + symbol.change / 100)
          : symbol.price *
            (trend +
              Math.sin(index * 0.66) * 0.025 +
              (noise(index, seed) - 0.5) * 0.025);
    const open = previous * (1 + (noise(index + 70, seed) - 0.5) * 0.018);
    const high = Math.max(open, close) * (1 + noise(index + 40, seed) * 0.017);
    const low = Math.min(open, close) * (1 - noise(index + 90, seed) * 0.017);
    const volume = Math.round(
      (2e6 + noise(index + 20, seed) * 8e6) *
        (1 + (Math.abs(close - open) / symbol.price) * 16),
    );
    const point = {
      time,
      minute: index,
      open,
      high,
      low,
      close,
      volume,
      turnover: (volume * (open + high + low + close)) / 4,
      average: close,
      previous,
    };
    previous = close;
    return point;
  });
}

function weekday(date: string) {
  return ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][
    new Date(`${date}T12:00:00Z`).getUTCDay()
  ];
}

export default function Chart({
  symbol,
  mode,
  onInspectDay,
  historical = false,
}: ChartProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 820, height: 430 });
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [visibleCount, setVisibleCount] = useState(120);
  const id = useId().replace(/:/g, "");
  const daily = mode === "daily";
  const points = useMemo(
    () => (daily ? makeDaily(symbol) : makeIntraday(symbol, historical)),
    [daily, historical, symbol.code, symbol.price, symbol.change],
  );
  const offset = daily ? Math.max(0, points.length - visibleCount) : 0;
  const visible = points.slice(offset);
  const averages = useMemo(
    () =>
      [5, 10, 20].map((period) =>
        points.map((_, index) => {
          const start = Math.max(0, index - period + 1);
          return (
            points
              .slice(start, index + 1)
              .reduce((sum, point) => sum + point.close, 0) /
            (index - start + 1)
          );
        }),
      ),
    [points],
  );

  useEffect(() => {
    const element = rootRef.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      if (width > 0 && height > 0) setSize({ width, height });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    setInspection(null);
    setVisibleCount(120);
  }, [symbol.code, mode, historical]);

  const width = Math.max(size.width, 220);
  const height = Math.max(size.height, 200);
  const compact = width < 600;
  const narrow = width < 400;
  const left = narrow ? 44 : 56;
  const right = width - (narrow ? 48 : 60);
  const top = 34;
  const bottom = height - 27;
  const volumeHeight = Math.max(48, Math.round(height * 0.19));
  const volumeTop = bottom - volumeHeight;
  const priceBottom = volumeTop - 29;
  const auctionWidth = compact ? 25 : 38;
  const morningStart = left + auctionWidth + (compact ? 8 : 13);
  const lunchGap = 10;
  const minuteWidth = (right - morningStart - lunchGap) / 240;
  const morningEnd = morningStart + minuteWidth * 120;
  const afternoonStart = morningEnd + lunchGap;
  const minuteX = (minute: number) =>
    minute <= 565
      ? left + ((minute - 555) / 10) * auctionWidth
      : minute <= 690
        ? morningStart + (minute - 570) * minuteWidth
        : afternoonStart + (minute - 780) * minuteWidth;
  const xAt = (index: number) =>
    daily
      ? left +
        5 +
        ((index - offset) / Math.max(1, visible.length - 1)) *
          (right - left - 10)
      : minuteX(points[index].minute);
  const previous = points[0].previous;
  const priceDelta =
    Math.max(
      previous * 0.025,
      ...points.map((point) => Math.abs(point.close - previous)),
    ) * 1.07;
  const dailyLow = Math.min(...visible.map((point) => point.low));
  const dailyHigh = Math.max(...visible.map((point) => point.high));
  const minPrice = daily
    ? dailyLow - (dailyHigh - dailyLow) * 0.08
    : previous - priceDelta;
  const maxPrice = daily
    ? dailyHigh + (dailyHigh - dailyLow) * 0.08
    : previous + priceDelta;
  const priceY = (value: number) =>
    top + ((maxPrice - value) / (maxPrice - minPrice)) * (priceBottom - top);
  const fromY = (y: number) =>
    maxPrice - ((y - top) / (priceBottom - top)) * (maxPrice - minPrice);
  const maxVolume = Math.max(...visible.map((point) => point.volume)) * 1.12;
  const gridDivisions = height < 280 ? 4 : 6;
  const volumeY = (value: number) =>
    bottom - (value / maxVolume) * volumeHeight;
  const pointPath = (values: number[]) =>
    values
      .map(
        (value, index) =>
          `${index === 0 ? "M" : "L"}${xAt(index + offset).toFixed(2)},${priceY(value).toFixed(2)}`,
      )
      .join(" ");
  // The opening-auction match remains separate from continuous trading.
  // The waiting period between 09:25 and 09:30 has no line or volume samples.
  const sessionPath = (field: "close" | "average") =>
    visible
      .slice(1)
      .map(
        (point, index) =>
          `${index ? "L" : "M"}${xAt(index + 1).toFixed(2)},${priceY(point[field]).toFixed(2)}`,
      )
      .join(" ");
  const intradayPath = sessionPath("close");
  const averagePath = sessionPath("average");
  const selected = inspection ? points[inspection.index] : undefined;
  // Period changes can temporarily retain an index from the longer intraday
  // series until the reset effect runs; never index MA arrays with that value.
  const inspectedIndex =
    selected && inspection ? inspection.index : points.length - 1;
  const latest = points[points.length - 1];
  const inspectedX = inspection ? xAt(inspection.index) : 0;
  const cardOnLeft = inspectedX > (left + right) / 2;

  const locate = (event: MouseEvent<SVGSVGElement>): Inspection | null => {
    const bounds = event.currentTarget.getBoundingClientRect();
    const x = ((event.clientX - bounds.left) / bounds.width) * width;
    const y = ((event.clientY - bounds.top) / bounds.height) * height;
    if (x < left || x > right || y < top || y > bottom) return null;
    if (
      !daily &&
      ((x > left + auctionWidth + 3 && x < morningStart - 2) ||
        (x > morningEnd + 2 && x < afternoonStart - 2) ||
        x > xAt(points.length - 1) + minuteWidth * 0.6)
    )
      return null;
    let closest = offset;
    let distance = Number.POSITIVE_INFINITY;
    for (let index = offset; index < points.length; index++) {
      const current = Math.abs(xAt(index) - x);
      if (current < distance) {
        closest = index;
        distance = current;
      }
    }
    // Indicative auction prices are not executed one-minute bars.
    if (!daily && x < left + auctionWidth - 4) return null;
    return { index: closest, y: clamp(y, top, priceBottom) };
  };
  const onClick = (event: MouseEvent<SVGSVGElement>) => {
    rootRef.current?.focus({ preventScroll: true });
    // Read the event during dispatch: React may replay the state updater after
    // currentTarget has been cleared (notably with StrictMode enabled).
    const hit = locate(event);
    setInspection((current) => (current ? null : hit));
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" && inspection) {
      event.stopPropagation();
      event.preventDefault();
      setInspection(null);
    } else if (
      (event.key === "ArrowLeft" || event.key === "ArrowRight") &&
      inspection
    ) {
      event.preventDefault();
      event.stopPropagation();
      const index = clamp(
        inspection.index + (event.key === "ArrowRight" ? 1 : -1),
        offset,
        points.length - 1,
      );
      setInspection({ index, y: priceY(points[index].close) });
    } else if ((event.key === "Enter" || event.key === " ") && !inspection) {
      event.preventDefault();
      setInspection({ index: points.length - 1, y: priceY(latest.close) });
    } else if (
      daily &&
      (event.key === "+" || event.key === "-" || event.key === "=")
    ) {
      event.preventDefault();
      setVisibleCount((count) =>
        clamp(count + (event.key === "-" ? 20 : -20), 40, 120),
      );
      setInspection(null);
    }
  };
  const rows = selected
    ? daily
      ? [
          {
            label: "开盘",
            value: fixed(selected.open),
            color: selected.open >= selected.previous ? UP : DOWN,
          },
          {
            label: "收盘",
            value: fixed(selected.close),
            color: selected.close >= selected.previous ? UP : DOWN,
          },
          { label: "最高", value: fixed(selected.high), color: UP },
          { label: "最低", value: fixed(selected.low), color: DOWN },
          {
            label: "涨跌幅",
            value: `${signed((selected.close / selected.previous - 1) * 100)}%`,
            color: selected.close >= selected.previous ? UP : DOWN,
          },
          { label: "成交量", value: `${quantity(selected.volume / 100)} 手` },
          { label: "成交额", value: `${quantity(selected.turnover)} 元` },
        ]
      : [
          {
            label: "最新",
            value: fixed(selected.close),
            color: selected.close >= previous ? UP : DOWN,
          },
          {
            label: "涨跌",
            value: signed(selected.close - previous),
            color: selected.close >= previous ? UP : DOWN,
          },
          {
            label: "涨跌幅",
            value: `${signed((selected.close / previous - 1) * 100)}%`,
            color: selected.close >= previous ? UP : DOWN,
          },
          { label: "均价", value: fixed(selected.average), color: GOLD },
          { label: "成交量", value: `${quantity(selected.volume / 100)} 手` },
          { label: "成交额", value: `${quantity(selected.turnover)} 元` },
        ]
    : [];
  const intradayTicks = compact
    ? [
        { x: morningStart, label: "09:30", anchor: "start" },
        {
          x: (morningEnd + afternoonStart) / 2,
          label: "11:30 / 13:00",
          anchor: "middle",
        },
        { x: right, label: "15:00", anchor: "end" },
      ]
    : [
        { x: left, label: "09:15", anchor: "middle" },
        { x: morningStart, label: "09:30", anchor: "middle" },
        { x: minuteX(630), label: "10:30", anchor: "middle" },
        {
          x: (morningEnd + afternoonStart) / 2,
          label: "11:30 / 13:00",
          anchor: "middle",
        },
        { x: minuteX(840), label: "14:00", anchor: "middle" },
        { x: right, label: "15:00", anchor: "end" },
      ];
  const dailyTicks = [0, 0.25, 0.5, 0.75, 1].map((ratio) => {
    const index = offset + Math.round((visible.length - 1) * ratio);
    return {
      x: xAt(index),
      label: points[index].time.slice(5).replace("-", "/"),
      anchor: ratio === 0 ? "start" : ratio === 1 ? "end" : "middle",
    };
  });

  return (
    <div
      className={`market-chart${narrow ? " is-narrow" : ""}`}
      ref={rootRef}
      tabIndex={0}
      onKeyDown={onKeyDown}
      data-testid="market-chart"
      role="application"
      aria-label={`${symbol.name}${daily ? "日线" : "分时"}图。单击开启或关闭十字线，方向键定位，Esc 退出${daily ? "，双击查看当日分时，按加减键缩放" : ""}。模拟行情。`}
    >
      <svg
        width="100%"
        height="100%"
        viewBox={`0 0 ${width} ${height}`}
        onClick={onClick}
        onMouseMove={(event) => {
          if (inspection) {
            const next = locate(event);
            if (next) setInspection(next);
          }
        }}
        onDoubleClick={(event) => {
          if (daily) {
            const hit = locate(event);
            if (hit) onInspectDay?.(points[hit.index].time);
          }
        }}
        aria-hidden="true"
      >
        <defs>
          <clipPath id={`${id}-plot`}>
            <rect
              x={left}
              y={top}
              width={right - left}
              height={priceBottom - top}
            />
          </clipPath>
          <linearGradient id={`${id}-area`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={BLUE} stopOpacity="0.08" />
            <stop offset="100%" stopColor={BLUE} stopOpacity="0.005" />
          </linearGradient>
        </defs>
        <g className="chart-svg-legend">
          {daily ? (
            [5, 10, 20].map((period, index) => (
              <text
                key={period}
                x={
                  left +
                  index * (narrow ? (width - left - 6) / 3 : compact ? 94 : 112)
                }
                y={17}
                fill={MA_COLORS[index]}
              >{`MA${period}  ${fixed(averages[index][inspectedIndex])}`}</text>
            ))
          ) : (
            <>
              <text x={left} y={17} fill={BLUE}>
                最新 {fixed(selected?.close ?? latest.close)}
              </text>
              <text x={left + 108} y={17} fill={GOLD}>
                均价 {fixed(selected?.average ?? latest.average)}
              </text>
            </>
          )}
          {!compact && (
            <text x={right} y={17} textAnchor="end" fill="#98a2b0">
              {daily
                ? "日 K · 模拟"
                : `${historical ? "历史分时" : "14:32:00"} · 元 / 手`}
            </text>
          )}
        </g>
        {!daily && (
          <>
            <rect
              x={left}
              y={top}
              width={auctionWidth}
              height={bottom - top}
              fill="#f5f8fb"
            />
            <rect
              x={morningEnd}
              y={top}
              width={lunchGap}
              height={bottom - top}
              fill="#f6f8fa"
            />
            <rect
              x={minuteX(897)}
              y={top}
              width={right - minuteX(897)}
              height={bottom - top}
              fill="#f0f6f5"
            />
            <text
              x={left + auctionWidth / 2}
              y={top + 13}
              textAnchor="middle"
              className="chart-session-label"
            >
              竞价
            </text>
            {!compact && (
              <text
                x={right - 3}
                y={priceBottom - 8}
                textAnchor="end"
                className="chart-session-label"
              >
                14:57 收盘竞价
              </text>
            )}
          </>
        )}
        {Array.from({ length: gridDivisions + 1 }, (_, row) => {
          const y = top + (row / gridDivisions) * (priceBottom - top);
          const value = fromY(y);
          const reference = !daily && row === gridDivisions / 2;
          const color = daily
            ? "#8793a3"
            : row < gridDivisions / 2
              ? "#d98094"
              : row > gridDivisions / 2
                ? "#67aaa6"
                : "#8793a3";
          return (
            <g key={row}>
              <line
                x1={left}
                x2={right}
                y1={y}
                y2={y}
                stroke={reference ? "#d4dce4" : "#edf1f5"}
                strokeDasharray={reference ? "4 4" : undefined}
              />
              <text
                x={left - 9}
                y={y + 3.5}
                textAnchor="end"
                fill={color}
                className="chart-axis"
              >
                {fixed(value)}
              </text>
              <text
                x={right + 9}
                y={y + 3.5}
                textAnchor="start"
                fill={color}
                className="chart-axis"
              >
                {daily
                  ? fixed(value)
                  : `${signed((value / previous - 1) * 100)}%`}
              </text>
            </g>
          );
        })}
        {(daily ? dailyTicks : intradayTicks).map((tick, index) => (
          <g key={index}>
            <line
              x1={tick.x}
              x2={tick.x}
              y1={top}
              y2={priceBottom}
              stroke="#edf1f5"
            />
            <line
              x1={tick.x}
              x2={tick.x}
              y1={volumeTop}
              y2={bottom}
              stroke="#f0f3f6"
            />
            <text
              x={tick.x}
              y={height - 8}
              textAnchor={tick.anchor as "start" | "middle" | "end"}
              className="chart-axis"
              fill="#8995a4"
            >
              {tick.label}
            </text>
          </g>
        ))}
        <g clipPath={`url(#${id}-plot)`}>
          {daily ? (
            <>
              {visible.map((point, localIndex) => {
                const x = xAt(localIndex + offset);
                const up = point.close >= point.open;
                const color = up ? UP : DOWN;
                const candleWidth = clamp(
                  ((right - left) / visible.length) * 0.63,
                  1.5,
                  8,
                );
                return (
                  <g key={point.time}>
                    <line
                      x1={x}
                      x2={x}
                      y1={priceY(point.high)}
                      y2={priceY(point.low)}
                      stroke={color}
                      strokeWidth="1"
                    />
                    <rect
                      x={x - candleWidth / 2}
                      y={priceY(Math.max(point.open, point.close))}
                      width={candleWidth}
                      height={Math.max(
                        1,
                        Math.abs(priceY(point.open) - priceY(point.close)),
                      )}
                      fill={up ? "#fff" : color}
                      stroke={color}
                      strokeWidth="1"
                    />
                  </g>
                );
              })}
              {averages.map((values, index) => (
                <path
                  key={index}
                  d={pointPath(values.slice(offset))}
                  stroke={MA_COLORS[index]}
                  strokeWidth="1.15"
                  fill="none"
                  opacity="0.9"
                />
              ))}
            </>
          ) : (
            <>
              <path
                d={`${intradayPath} L${xAt(points.length - 1)},${priceBottom} L${xAt(1)},${priceBottom} Z`}
                fill={`url(#${id}-area)`}
              />
              <path
                d={intradayPath}
                fill="none"
                stroke={BLUE}
                strokeWidth="1.6"
                strokeLinejoin="round"
                strokeLinecap="round"
              />
              <path
                d={averagePath}
                fill="none"
                stroke={GOLD}
                strokeWidth="1.3"
                strokeLinejoin="round"
              />
              <path
                d={Array.from(
                  { length: 11 },
                  (_, i) =>
                    `${i ? "L" : "M"}${left + (i / 10) * auctionWidth},${priceY(previous + ((points[0].close - previous) * i) / 10 + previous * Math.sin(i * 1.4) * 0.0015)}`,
                ).join(" ")}
                fill="none"
                stroke="#7da2cd"
                strokeWidth="1"
                strokeDasharray="2 2"
              />
              <circle
                cx={xAt(0)}
                cy={priceY(points[0].close)}
                r="2"
                fill={BLUE}
              />
              <circle
                cx={xAt(points.length - 1)}
                cy={priceY(latest.close)}
                r="3"
                fill={BLUE}
                stroke="#fff"
                strokeWidth="1.5"
              />
            </>
          )}
        </g>
        <text
          x={left}
          y={priceBottom + 19}
          className="chart-volume-label"
          fill="#8c98a6"
        >
          成交量{" "}
          <tspan fill="#536478">
            {quantity((selected?.volume ?? latest.volume) / 100)} 手
          </tspan>
        </text>
        <text
          x={right}
          y={priceBottom + 19}
          textAnchor="end"
          className="chart-volume-label"
          fill="#a2abb7"
        >
          {daily ? "MA5 / MA10" : "量随价动"}
        </text>
        {[0, 0.5, 1].map((ratio) => (
          <g key={ratio}>
            <line
              x1={left}
              x2={right}
              y1={volumeTop + ratio * volumeHeight}
              y2={volumeTop + ratio * volumeHeight}
              stroke="#edf1f5"
            />
            {ratio < 1 && (
              <text
                x={left - 9}
                y={volumeTop + ratio * volumeHeight + 4}
                textAnchor="end"
                className="chart-axis"
                fill="#99a3af"
              >
                {quantity((maxVolume * (1 - ratio)) / 100)}
              </text>
            )}
          </g>
        ))}
        {visible.map((point, localIndex) => (
          <rect
            key={point.time}
            x={xAt(localIndex + offset) - (daily ? 1.8 : 0.8)}
            y={volumeY(point.volume)}
            width={
              daily
                ? clamp(((right - left) / visible.length) * 0.55, 1, 7)
                : clamp(minuteWidth * 0.7, 1, 3)
            }
            height={bottom - volumeY(point.volume)}
            fill={point.close >= point.open ? UP : DOWN}
            opacity={daily ? 0.76 : 0.62}
          />
        ))}
        {daily &&
          [5, 10].map((period, periodIndex) => (
            <path
              key={period}
              d={visible
                .map((_, localIndex) => {
                  const index = offset + localIndex;
                  const from = Math.max(0, index - period + 1);
                  const value =
                    points
                      .slice(from, index + 1)
                      .reduce((sum, point) => sum + point.volume, 0) /
                    (index - from + 1);
                  return `${localIndex ? "L" : "M"}${xAt(index)},${volumeY(value)}`;
                })
                .join(" ")}
              stroke={MA_COLORS[periodIndex]}
              strokeWidth="1"
              fill="none"
              opacity="0.85"
            />
          ))}
        {inspection && selected && (
          <g data-testid="chart-crosshair">
            <line
              x1={inspectedX}
              x2={inspectedX}
              y1={top}
              y2={bottom}
              stroke="#6f849a"
              strokeWidth="0.8"
              strokeDasharray="4 4"
            />
            <line
              x1={left}
              x2={right}
              y1={inspection.y}
              y2={inspection.y}
              stroke="#6f849a"
              strokeWidth="0.8"
              strokeDasharray="4 4"
            />
            <rect
              x={3}
              y={inspection.y - 9}
              width={left - 6}
              height={18}
              rx="3"
              fill="#627b94"
            />
            <text
              x={left - 7}
              y={inspection.y + 3.5}
              fill="#fff"
              textAnchor="end"
              className="chart-axis"
            >
              {fixed(fromY(inspection.y))}
            </text>
            <circle
              cx={inspectedX}
              cy={priceY(selected.close)}
              r="3.5"
              fill={BLUE}
              stroke="#fff"
              strokeWidth="1.5"
            />
            <rect
              x={clamp(
                inspectedX - (daily ? 39 : 23),
                left,
                right - (daily ? 78 : 46),
              )}
              y={bottom + 4}
              width={daily ? 78 : 46}
              height="19"
              rx="3"
              fill="#627b94"
            />
            <text
              x={clamp(
                inspectedX,
                left + (daily ? 39 : 23),
                right - (daily ? 39 : 23),
              )}
              y={bottom + 17}
              fill="#fff"
              textAnchor="middle"
              className="chart-axis"
            >
              {selected.time}
            </text>
          </g>
        )}
      </svg>
      {selected && (
        <div
          className={`chart-inspection ${cardOnLeft ? "is-left" : "is-right"}`}
          data-testid="inspection-card"
          style={cardOnLeft ? { left: left + 9 } : { right: width - right + 9 }}
          onClick={() => setInspection(null)}
          aria-live="polite"
        >
          <div className="chart-inspection-heading">
            <strong>{selected.time}</strong>
            <span>{daily ? weekday(selected.time) : "分时明细"}</span>
          </div>
          {rows.map((row) => (
            <div className="chart-inspection-row" key={row.label}>
              <span>{row.label}</span>
              <strong style={{ color: row.color }}>{row.value}</strong>
            </div>
          ))}
          <div className="chart-inspection-foot">
            {daily ? "双击蜡烛 · 查看当日分时" : "再次点击 · 退出十字线"}
          </div>
        </div>
      )}
    </div>
  );
}
