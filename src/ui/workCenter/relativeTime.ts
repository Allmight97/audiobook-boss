export function formatRelativeTime(thenMs: number, nowMs: number): string {
	const elapsedMs = Math.max(0, nowMs - thenMs);
	const elapsedSeconds = elapsedMs / 1_000;
	if (elapsedSeconds < 45) return 'just now';

	const elapsedMinutes = elapsedMs / 60_000;
	if (elapsedMinutes < 60) return `${Math.max(1, Math.floor(elapsedMinutes))}m ago`;

	const elapsedHours = elapsedMs / 3_600_000;
	if (elapsedHours < 24) return `${Math.floor(elapsedHours)}h ago`;

	return `${Math.floor(elapsedMs / 86_400_000)}d ago`;
}
