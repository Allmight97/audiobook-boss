import { beforeEach, describe, expect, it, vi } from 'vitest';

const { initializeMock } = vi.hoisted(() => ({
	initializeMock: vi.fn(),
}));

vi.mock('../encoderPanel/logic', () => ({
	initializeEncoderPanelLogic: initializeMock,
}));

import { initEncoderPanel } from '../encoderPanel';

describe('EncoderPanel island mount', () => {
	beforeEach(() => {
		initializeMock.mockReset();
		document.body.innerHTML = '<div id="encoder-panel-root"></div>';
	});

	it('mounts encoder controls and runs panel initialization logic', () => {
		initEncoderPanel();

		expect(document.getElementById('encoder-settings-panel')).toBeTruthy();
		expect(document.getElementById('adv-encoder')).toBeTruthy();
		expect(initializeMock).toHaveBeenCalledTimes(1);
	});
});
