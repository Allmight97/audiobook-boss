import { describe, expect, it } from 'vitest';

import * as processing from '.';

const EXPECTED_APP_PROCESSING_EXPORTS = ['createProcessingOwner'] as const;

describe('app Processing Public API Strip', () => {
	it('pins the app processing public export strip', () => {
		expect(Object.keys(processing).sort()).toEqual([...EXPECTED_APP_PROCESSING_EXPORTS].sort());
	});

	it('does not export bind slots, status-panel singletons, or live workflow symbols', () => {
		expect(processing).not.toHaveProperty('bindProcessingInput');
		expect(processing).not.toHaveProperty('bindProcessingMetadata');
		expect(processing).not.toHaveProperty('bindProcessingSettings');
		expect(processing).not.toHaveProperty('initStatusPanel');
		expect(processing).not.toHaveProperty('getStatusView');
		expect(processing).not.toHaveProperty('readProcessingRequestConfig');
		expect(processing).not.toHaveProperty('makeProcessingWorkflowServicesLayer');
		expect(processing).not.toHaveProperty('triggerProcessFromStatusPanel');
		expect(processing).not.toHaveProperty('setProcessingEncodingReader');
	});
});
