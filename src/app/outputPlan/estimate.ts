import { formatFileSize } from '../../types/audio';

export type EstimateEncoderRequest = {
	readonly bitrateKbps: number | null;
	readonly channels: string;
};

export function estimateEncodedSizeBytes(
	durationSeconds: number,
	request: EstimateEncoderRequest,
): number {
	if (!durationSeconds || durationSeconds <= 0 || request.bitrateKbps === null) {
		return 0;
	}

	let sizeBytes = (durationSeconds * request.bitrateKbps * 1000) / 8;
	sizeBytes *= 1.03;
	return Math.round(sizeBytes);
}

export function formatEstimatedSizeText(
	hasFiles: boolean,
	durationSeconds: number,
	request: EstimateEncoderRequest,
): string {
	if (!hasFiles) {
		return '~ --- MB';
	}
	if (request.bitrateKbps === null) return 'Size depends on audio';
	return `~ ${formatFileSize(estimateEncodedSizeBytes(durationSeconds, request))}`;
}
