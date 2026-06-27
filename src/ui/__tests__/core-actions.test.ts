import { beforeEach, describe, expect, it, vi } from 'vitest';

const context = vi.hoisted(() => {
	return {
		initStatusPanelMock: vi.fn(),
		isStatusPanelProcessingMock: vi.fn(() => false),
		triggerProcessFromStatusPanelMock: vi.fn(),
	};
});

vi.mock('../statusPanel/index', () => ({
	initStatusPanel: context.initStatusPanelMock,
	isStatusPanelProcessing: context.isStatusPanelProcessingMock,
	pushStatusPanelTransientStatus: vi.fn(),
	triggerProcessFromStatusPanel: context.triggerProcessFromStatusPanelMock,
}));

describe('core actions', () => {
	beforeEach(() => {
		vi.resetModules();
		context.initStatusPanelMock.mockReset();
		context.isStatusPanelProcessingMock.mockReset();
		context.triggerProcessFromStatusPanelMock.mockReset();
		context.isStatusPanelProcessingMock.mockReturnValue(false);
	});

	it('routes preview dispatch through the status-panel runtime trigger', async () => {
		const { startPreviewAudio } = await import('../core/actions');

		startPreviewAudio(15);
		startPreviewAudio(30);

		expect(context.triggerProcessFromStatusPanelMock).toHaveBeenNthCalledWith(1, {
			previewSeconds: 15,
		});
		expect(context.triggerProcessFromStatusPanelMock).toHaveBeenNthCalledWith(2, {
			previewSeconds: 30,
		});
	});
});
