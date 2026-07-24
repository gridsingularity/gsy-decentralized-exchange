import { useEffect, useState } from 'react';
import { getHealth, getTrades } from './api/client';
import type { TradeSchema } from './api/schema';

function truncate(value: string, head = 8, tail = 6): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

const cellStyle: React.CSSProperties = {
  border: '1px solid #ddd',
  padding: '0.35rem 0.5rem',
  fontFamily: 'monospace',
};

export default function App() {
  const [healthy, setHealthy] = useState<boolean | null>(null);
  const [trades, setTrades] = useState<TradeSchema[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      setError(null);
      try {
        const [health, tradeList] = await Promise.all([
          getHealth(),
          getTrades(),
        ]);
        if (cancelled) return;
        setHealthy(health);
        setTrades(tradeList);
      } catch (err) {
        if (cancelled) return;
        setHealthy(false);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const badgeColor =
    healthy === null ? '#888' : healthy ? '#1a7f37' : '#cf222e';
  const badgeLabel = healthy === null ? '…' : healthy ? 'OK' : 'ERROR';

  return (
    <main
      style={{
        fontFamily: 'sans-serif',
        padding: '1.5rem',
        maxWidth: 1200,
        margin: '0 auto',
      }}
    >
      <header
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '0.75rem',
          marginBottom: '1rem',
        }}
      >
        <h1 style={{ fontSize: '1.4rem', margin: 0 }}>GSY DEX Admin</h1>
        <span
          style={{
            background: badgeColor,
            color: '#fff',
            borderRadius: 4,
            padding: '0.15rem 0.5rem',
            fontSize: '0.8rem',
            fontWeight: 600,
          }}
        >
          health_check: {badgeLabel}
        </span>
      </header>

      {loading && <p>Loading…</p>}

      {error && (
        <pre
          style={{
            background: '#fff0f0',
            border: '1px solid #cf222e',
            color: '#cf222e',
            padding: '0.75rem',
            borderRadius: 4,
            whiteSpace: 'pre-wrap',
          }}
        >
          {error}
        </pre>
      )}

      {trades && (
        <>
          <p style={{ color: '#555' }}>{trades.length} trade(s)</p>
          <table
            style={{
              borderCollapse: 'collapse',
              width: '100%',
              fontSize: '0.85rem',
            }}
          >
            <thead>
              <tr style={{ textAlign: 'left', background: '#f0f0f0' }}>
                <th style={cellStyle}>time_slot</th>
                <th style={cellStyle}>market_id</th>
                <th style={cellStyle}>seller</th>
                <th style={cellStyle}>buyer</th>
                <th style={cellStyle}>selected_energy</th>
                <th style={cellStyle}>energy_rate</th>
                <th style={cellStyle}>status</th>
              </tr>
            </thead>
            <tbody>
              {trades.map((t) => (
                <tr key={t._id}>
                  <td style={cellStyle}>{t.time_slot}</td>
                  <td style={cellStyle} title={t.market_id}>
                    {truncate(t.market_id)}
                  </td>
                  <td style={cellStyle} title={t.seller}>
                    {truncate(t.seller)}
                  </td>
                  <td style={cellStyle} title={t.buyer}>
                    {truncate(t.buyer)}
                  </td>
                  <td style={cellStyle}>{t.parameters.selected_energy}</td>
                  <td style={cellStyle}>{t.parameters.energy_rate}</td>
                  <td style={cellStyle}>{t.status}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </main>
  );
}
