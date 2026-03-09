export function showProgress(
  current: number,
  total: number,
  startTime: number,
  label = "Progress",
): void {
  const elapsed = Math.max((Date.now() - startTime) / 1000, 0.001);
  const pct = ((current / total) * 100).toFixed(1);
  const bps = (current / elapsed).toFixed(1);
  const remaining =
    current > 0
      ? (((total - current) / (current / elapsed)) / 60).toFixed(1)
      : "?";

  process.stdout.write(
    `\r  ${label}: ${current}/${total} (${pct}%) | ${bps} blocks/s | ETA: ${remaining}m`,
  );
}

export function clearProgress(): void {
  process.stdout.write("\r\x1b[2K");
}
