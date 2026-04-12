import { beforeEach, describe, expect, it, vi } from 'vitest';

const context = vi.hoisted(() => {
	const statusPanel = {
		isCurrentlyProcessing: false,
		startProcessing: vi.fn(),
	};

	return {
		statusPanel,
		initStatusPanelMock: vi.fn(() => statusPanel),
	};
});

vi.mock('../fileImport', () => ({
	initFileImport: vi.fn(),
}));

vi.mock('../encoderPanel', () => ({
	initEncoderPanel: vi.fn(),
}));

vi.mock('../outputPanel', () => ({
	initOutputPanel: vi.fn(),
}));

vi.mock('../statusPanel/index', () => ({
	initStatusPanel: context.initStatusPanelMock,
	pushStatusPanelTransientStatus: vi.fn(),
}));

vi.mock('../coverArt', () => ({
	initCoverArt: vi.fn(),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
	onClearCoverArt: vi.fn(),
}));

vi.mock('../metadataForm', () => ({
	initMetadataFormEvents: vi.fn(),
	onMetadataFormActionSelectChange: vi.fn(),
	onMetadataFormFieldInput: vi.fn(),
	resetDirtyState: vi.fn(),
	setMetadataFormSaveHandler: vi.fn(),
	triggerMetadataFormSave: vi.fn(),
}));

vi.mock('../tagPreview', () => ({
	initTagPreview: vi.fn(),
}));

vi.mock('../metadataLookup', () => ({
	initMetadataLookup: vi.fn(),
}));

vi.mock('../jobControls', () => ({
	initJobControls: vi.fn(),
	handleMergeModeChange: vi.fn(),
	handleMaxConcurrentSelectionChange: vi.fn(),
}));

describe('bootstrap status panel seam', () => {
	beforeEach(() => {
		vi.resetModules();
		context.initStatusPanelMock.mockClear();
		context.statusPanel.isCurrentlyProcessing = false;
		context.statusPanel.startProcessing.mockClear();
		document.body.innerHTML = '<div id="app"></div>';
	});

	it('initializes the status panel once and reuses the cached handle for preview dispatch', async () => {
		const { initializeAppShell, startPreviewAudio } = await import('../core/bootstrap');

		expect(initializeAppShell()).toBeNull();
		startPreviewAudio(15);
		startPreviewAudio(30);

		expect(context.initStatusPanelMock).toHaveBeenCalledTimes(1);
		expect(context.statusPanel.startProcessing).toHaveBeenCalledTimes(2);
		expect(context.statusPanel.startProcessing).toHaveBeenNthCalledWith(1, {
			previewSeconds: 15,
		});
		expect(context.statusPanel.startProcessing).toHaveBeenNthCalledWith(2, {
			previewSeconds: 30,
		});
	});
});
