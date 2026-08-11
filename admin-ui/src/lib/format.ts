// Shared formatting helpers used across views.

/** Middle-ellipsis truncation for long ids (uuids, hashes, account strings). */
export function truncate(value: string, head = 8, tail = 6): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}
