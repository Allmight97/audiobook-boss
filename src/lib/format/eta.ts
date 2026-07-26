/** Mock transport vocabulary: "04:02 left" (H:MM:SS past the hour). */
export function formatEtaRemaining(etaSeconds: number): string {
	const total = Math.max(0, Math.round(etaSeconds));
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor((total % 3600) / 60);
	const seconds = total % 60;
	const mm = String(minutes).padStart(2, '0');
	const ss = String(seconds).padStart(2, '0');
	return hours > 0 ? `${hours}:${mm}:${ss} left` : `${mm}:${ss} left`;
}
