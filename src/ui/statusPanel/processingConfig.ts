import type { ProcessingRequestConfig } from '../../types/audio';
import { readEncodingRequestConfig } from '../encoderPanel';
import { readOutputRequestConfig } from '../outputPanel';

export function readProcessingRequestConfig(): ProcessingRequestConfig {
	return {
		...readEncodingRequestConfig(),
		...readOutputRequestConfig(),
	};
}
