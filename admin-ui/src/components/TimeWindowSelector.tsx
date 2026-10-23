// Global time-window selector. Two datetime-local controls (start / end)
// whose values are epoch seconds snapped to 900s slot boundaries. The parent
// owns the URL sync; this component just renders the current window and emits
// snapped changes.

import {
  epochToLocalInput,
  formatSlot,
  localInputToEpoch,
  type Window,
} from '../lib/time';

interface Props {
  window: Window;
  onChange: (next: Window) => void;
}

export default function TimeWindowSelector({ window, onChange }: Props) {
  const handleStart = (value: string) => {
    const start = localInputToEpoch(value);
    if (start === null) return;
    onChange({ start, end: Math.max(start, window.end) });
  };

  const handleEnd = (value: string) => {
    const end = localInputToEpoch(value);
    if (end === null) return;
    onChange({ start: Math.min(window.start, end), end });
  };

  return (
    <section className="tw-selector">
      <div className="tw-field">
        <label htmlFor="tw-start">Start</label>
        <input
          id="tw-start"
          type="datetime-local"
          value={epochToLocalInput(window.start)}
          onChange={(e) => handleStart(e.target.value)}
        />
      </div>
      <div className="tw-field">
        <label htmlFor="tw-end">End</label>
        <input
          id="tw-end"
          type="datetime-local"
          value={epochToLocalInput(window.end)}
          onChange={(e) => handleEnd(e.target.value)}
        />
      </div>
      <div className="tw-summary">
        <span title="epoch seconds (snapped to 900s)">
          {window.start} … {window.end}
        </span>
        <span className="tw-human">
          {formatSlot(window.start)} → {formatSlot(window.end)}
        </span>
      </div>
    </section>
  );
}
