// Minimal, self-contained inline-SVG bar chart (no charting dependency).
//
// Renders labeled bars with a y-axis max label and a per-bar <title> tooltip.
// Handles the empty and all-zero cases gracefully.

export interface BarPoint {
  label: string;
  value: number;
  /** Optional flag to visually mark a bar (e.g. the selected slot). */
  highlight?: boolean;
}

interface Props {
  points: BarPoint[];
  title: string;
  unit: string;
}

const WIDTH = 320;
const HEIGHT = 140;
const PAD_LEFT = 6;
const PAD_RIGHT = 6;
const PAD_TOP = 10;
const PLOT_BOTTOM = 108; // baseline y; below is label space
const PLOT_TOP = PAD_TOP;

function fmt(n: number): string {
  if (!Number.isFinite(n)) return '0';
  const abs = Math.abs(n);
  if (abs !== 0 && abs < 0.01) return n.toExponential(1);
  return n.toFixed(abs >= 100 ? 0 : 2);
}

export default function SlotBarChart({ points, title, unit }: Props) {
  const max = points.reduce((m, p) => Math.max(m, p.value), 0);
  const hasData = points.length > 0;
  const plotH = PLOT_BOTTOM - PLOT_TOP;
  const plotW = WIDTH - PAD_LEFT - PAD_RIGHT;
  const slot = hasData ? plotW / points.length : plotW;
  const barW = Math.max(2, slot * 0.62);

  return (
    <figure className="bar-chart">
      <figcaption className="bar-chart-title">
        {title} <span className="muted">({unit})</span>
      </figcaption>
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        width="100%"
        role="img"
        aria-label={`${title} per slot`}
        className="bar-chart-svg"
      >
        {/* y-axis max label + baseline */}
        <line
          x1={PAD_LEFT}
          y1={PLOT_BOTTOM}
          x2={WIDTH - PAD_RIGHT}
          y2={PLOT_BOTTOM}
          className="bar-chart-axis"
        />
        {max > 0 && (
          <text x={PAD_LEFT} y={PLOT_TOP - 1} className="bar-chart-axis-label">
            max {fmt(max)} {unit}
          </text>
        )}

        {!hasData && (
          <text x={WIDTH / 2} y={HEIGHT / 2} className="bar-chart-empty">
            no data
          </text>
        )}
        {hasData && max === 0 && (
          <text x={WIDTH / 2} y={HEIGHT / 2} className="bar-chart-empty">
            all zero
          </text>
        )}

        {hasData &&
          points.map((p, i) => {
            const h = max > 0 ? (p.value / max) * plotH : 0;
            const x = PAD_LEFT + i * slot + (slot - barW) / 2;
            const y = PLOT_BOTTOM - h;
            return (
              <g key={`${p.label}-${i}`}>
                <rect
                  x={x}
                  y={y}
                  width={barW}
                  height={h}
                  rx={1.5}
                  className={
                    p.highlight ? 'bar-chart-bar selected' : 'bar-chart-bar'
                  }
                >
                  <title>{`${p.label}: ${fmt(p.value)} ${unit}`}</title>
                </rect>
                <text
                  x={x + barW / 2}
                  y={HEIGHT - 4}
                  className="bar-chart-x-label"
                  textAnchor="middle"
                >
                  {p.label}
                </text>
              </g>
            );
          })}
      </svg>
    </figure>
  );
}
