// Measurements vs forecasts for a single asset (Milestone 5).
//
// ⚠ join caveat (admin-ui plan §9.4): the /measurements and /forecasts
// endpoints filter on their `area_uuid` field, which in the current seed data
// carries a SYNTHETIC sensor id (e.g. `<Community>_asset_N`) that matches
// NEITHER the topology asset `name` (`..._pv` / `..._meter`) NOR any hash.
// We query by the topology asset `name` (the only stable identity we have);
// today that correctly returns empty for these assets, so we render an
// explanatory diagnostic rather than an error.

import { useEffect, useState } from 'react';
import { getForecasts, getMeasurements } from '../api/client';
import type { ForecastSchema, MeasurementSchema } from '../api/schema';
import { formatSlot, type Window } from '../lib/time';
import { ErrorNote, Loading } from './AsyncState';

interface Props {
  assetName: string;
  window: Window;
}

interface State {
  loading: boolean;
  error: string | null;
  measurements: MeasurementSchema[];
  forecasts: ForecastSchema[];
}

interface JoinedRow {
  time_slot: number;
  measured: number | null;
  forecast: number | null;
  confidence: number | null;
}

/** Join measurements and forecasts on `time_slot`, sorted ascending. */
function joinOnSlot(
  measurements: MeasurementSchema[],
  forecasts: ForecastSchema[],
): JoinedRow[] {
  const bySlot = new Map<number, JoinedRow>();
  const ensure = (ts: number): JoinedRow => {
    let row = bySlot.get(ts);
    if (!row) {
      row = { time_slot: ts, measured: null, forecast: null, confidence: null };
      bySlot.set(ts, row);
    }
    return row;
  };
  for (const m of measurements) ensure(m.time_slot).measured = m.energy_kwh;
  for (const f of forecasts) {
    const row = ensure(f.time_slot);
    row.forecast = f.energy_kwh;
    row.confidence = f.confidence;
  }
  return [...bySlot.values()].sort((a, b) => a.time_slot - b.time_slot);
}

export default function MeasurementForecastPanel({ assetName, window }: Props) {
  const [state, setState] = useState<State>({
    loading: true,
    error: null,
    measurements: [],
    forecasts: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, error: null }));
    (async () => {
      try {
        const [measurements, forecasts] = await Promise.all([
          getMeasurements({
            area_uuid: assetName,
            start_time: window.start,
            end_time: window.end,
          }),
          getForecasts({
            area_uuid: assetName,
            start_time: window.start,
            end_time: window.end,
          }),
        ]);
        if (cancelled) return;
        setState({ loading: false, error: null, measurements, forecasts });
      } catch (err) {
        if (cancelled) return;
        setState({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          measurements: [],
          forecasts: [],
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [assetName, window.start, window.end]);

  if (state.loading) {
    return <Loading label="Loading measurements & forecasts…" />;
  }
  if (state.error) {
    return <ErrorNote error={state.error} />;
  }

  if (state.measurements.length === 0 && state.forecasts.length === 0) {
    return (
      <p className="banner">
        No measurements/forecasts linked to this asset. In the current data,
        measurement/forecast <code>area_uuid</code> values (e.g.{' '}
        <code>&lt;Community&gt;_asset_N</code>) don't match topology asset names
        (queried here as <code>{assetName}</code>) — see plan §9.4.
      </p>
    );
  }

  const rows = joinOnSlot(state.measurements, state.forecasts);

  return (
    <div>
      <p className="caption muted">
        Measured <code>energy_kwh</code> vs forecast <code>energy_kwh</code> (±
        confidence), joined on <code>time_slot</code>.
      </p>
      <table className="data-table">
        <thead>
          <tr>
            <th>Slot</th>
            <th className="num">Measured (kWh)</th>
            <th className="num">Forecast (kWh)</th>
            <th className="num">Confidence</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.time_slot}>
              <td>{formatSlot(r.time_slot)}</td>
              <td className="num">
                {r.measured === null ? '—' : r.measured.toFixed(3)}
              </td>
              <td className="num">
                {r.forecast === null ? '—' : r.forecast.toFixed(3)}
              </td>
              <td className="num">
                {r.confidence === null ? '—' : r.confidence.toFixed(2)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
