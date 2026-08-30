import type { ProcessingRequestConfig } from '../../types/audio';
import { readOutputRequestConfig } from '../outputPlan';
import { boundProcessingEncoding } from './bind';

export function readProcessingRequestConfig(): ProcessingRequestConfig {
	const encoding = boundProcessingEncoding()?.();
	if (!encoding) {
		throw new Error('Processing runtime is not bound');
	}
	return {
		...encoding,
		...readOutputRequestConfig(),
	};
}
