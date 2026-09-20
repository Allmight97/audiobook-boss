import { formatFileSize } from '../../types/audio';

export type EstimateEncoderRequest = {
	readonly bitrateKbps: number | null;
};

export function estimateEncodedSizeBytes(
	durationSeconds: number,
	request: EstimateEncoderRequest,
): number | null {
	if (!durationSeconds || durationSeconds <= 0) {
		return 0;
	}

	if (request.bitrateKbps === null) return null;
	let sizeBytes = (durationSeconds * request.bitrateKbps * 1000) / 8;
	sizeBytes *= 1.03;
	return Math.round(sizeBytes);
}

export function formatEstimatedSizeText(
	hasFiles: boolean,
	durationSeconds: number,
	request: EstimateEncoderRequest,
	preservedBytes = 0,
): string {
	if (!hasFiles) {
		return '~ --- MB';
	}
	const encodedBytes = estimateEncodedSizeBytes(durationSeconds, request);
	return encodedBytes === null
		? 'Size varies with audio'
		: `~ ${formatFileSize(preservedBytes + encodedBytes)}`;
}
