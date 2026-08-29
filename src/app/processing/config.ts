import type { ProcessingRequestConfig } from '../../types/audio';
import { encodingRequestConfigAtom } from '../../ui/encoderPanel/requestConfig';
import { readOutputRequestConfig } from '../outputPlan';
import { tryProcessingRegistry } from './registry';

export function readProcessingRequestConfig(): ProcessingRequestConfig {
	const registry = tryProcessingRegistry();
	if (!registry) {
		throw new Error('Output directory not selected');
	}
	return {
		...registry.get(encodingRequestConfigAtom),
		...readOutputRequestConfig(),
	};
}
