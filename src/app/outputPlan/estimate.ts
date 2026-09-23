export function estimateEncodedSizeBytes(durationSeconds: number, bitrateKbps: number): number {
	if (durationSeconds <= 0) return 0;
	return Math.round(((durationSeconds * bitrateKbps * 1000) / 8) * 1.03);
}
