// Centralized slot <-> date helpers and time-window handling.
//
// All time bounds sent to the backend are Unix *seconds* on a 900s (15-min)
// grid. The query endpoints type their bounds as u32, so we must never send a
// value > u32::MAX. Market `time_slot` is u32; trade/order slots are u64 but
// still fit safely in a JS number (exact up to 2^53). This module is the single
// place where that u32/u64 handling lives so it never leaks into view code.

export const SLOT_SECONDS = 900;
export const U32_MAX = 4294967295; // 2^32 - 1

const DAY_SECONDS = 86400;

export interface Window {
  start: number;
  end: number;
}

/** Current wall-clock time in whole Unix seconds. */
export function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

/** Clamp a seconds value into the u32 range the backend accepts. */
export function clampU32(sec: number): number {
  if (!Number.isFinite(sec)) return 0;
  return Math.min(Math.max(0, Math.floor(sec)), U32_MAX);
}

/** Snap a seconds value down to the nearest 900s slot boundary (and clamp). */
export function snapToSlot(sec: number): number {
  const clamped = clampU32(sec);
  return clamped - (clamped % SLOT_SECONDS);
}

// Default window. The live/seeded data's time_slots sit around ~1.784e9
// (mid-2026), so a window anchored on "now" surfaces them. We deliberately go
// wide: [now - 45 days, now + 1 day]. If this ever returned empty against the
// backend it should be widened further (see admin-ui plan / milestone notes).
export function defaultWindow(): Window {
  const now = nowSeconds();
  return {
    start: snapToSlot(now - 45 * DAY_SECONDS),
    end: snapToSlot(now + 1 * DAY_SECONDS),
  };
}

/**
 * Parse a start/end pair coming from URL query params. Falls back to the
 * default window for any missing/invalid value, and guarantees start <= end.
 */
export function parseWindow(
  startRaw: string | undefined,
  endRaw: string | undefined,
): Window {
  const def = defaultWindow();
  const start = coerceSeconds(startRaw, def.start);
  const end = coerceSeconds(endRaw, def.end);
  if (start > end) return { start: end, end: start };
  return { start, end };
}

function coerceSeconds(raw: string | undefined, fallback: number): number {
  if (raw === undefined) return fallback;
  const n = Number(raw);
  if (!Number.isFinite(n) || n <= 0) return fallback;
  return snapToSlot(n);
}

// --- datetime-local <-> epoch seconds -----------------------------------
// <input type="datetime-local"> works in local time with a "YYYY-MM-DDTHH:mm"
// string (no timezone). These helpers convert to/from epoch seconds.

function pad(n: number): string {
  return String(n).padStart(2, '0');
}

/** Epoch seconds -> "YYYY-MM-DDTHH:mm" in the browser's local timezone. */
export function epochToLocalInput(sec: number): string {
  const d = new Date(sec * 1000);
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` +
    `T${pad(d.getHours())}:${pad(d.getMinutes())}`
  );
}

/** "YYYY-MM-DDTHH:mm" (local time) -> epoch seconds, snapped to a slot. */
export function localInputToEpoch(value: string): number | null {
  const ms = new Date(value).getTime();
  if (!Number.isFinite(ms)) return null;
  return snapToSlot(Math.floor(ms / 1000));
}

/** Human-readable rendering of a slot (epoch seconds) in local time. */
export function formatSlot(sec: number): string {
  const d = new Date(sec * 1000);
  return d.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
