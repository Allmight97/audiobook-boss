import { formatFileSize } from '../../types/audio';

export type EstimateEncoderRequest = {
	readonly bitrateKbps: number;
	readonly channels: string;
};

export function estimateEncodedSizeBytes(
	durationSeconds: number,
	request: EstimateEncoderRequest,
): number {
	if (!durationSeconds || durationSeconds <= 0) {
		return 0;
	}

	let sizeBytes = (durationSeconds * request.bitrateKbps * 1000) / 8;
	if (request.channels === 'stereo') {
		sizeBytes *= 1.5;
	}
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
	return `~ ${formatFileSize(estimateEncodedSizeBytes(durationSeconds, request))}`;
}
