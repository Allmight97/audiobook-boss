import type { EncodingRequestConfig, ProcessingRequestConfig } from '../../types/audio';
import { readOutputRequestConfig } from '../outputPlan';

let readEncoding: (() => EncodingRequestConfig) | undefined;

export function setProcessingEncodingReader(read: (() => EncodingRequestConfig) | undefined): void {
	readEncoding = read;
}

export function readProcessingRequestConfig(): ProcessingRequestConfig {
	const encoding = readEncoding?.();
	if (!encoding) {
		throw new Error('Processing runtime is not bound');
	}
	return {
		...encoding,
		...readOutputRequestConfig(),
	};
}
