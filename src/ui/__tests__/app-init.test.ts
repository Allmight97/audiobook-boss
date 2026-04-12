import { beforeEach, describe, expect, it, vi } from 'vitest';

const context = vi.hoisted(() => ({
	initOrder: [] as string[],
	failInitAt: null as string | null,
}));

function runInit(name: string): void {
	context.initOrder.push(name);
	if (context.failInitAt === name) {
		throw new Error(`${name} init failed`);
	}
}

vi.mock('../fileImport', () => ({
	initFileImport: vi.fn(() => runInit('fileImport')),
}));

vi.mock('../encoderPanel', () => ({
	initEncoderPanel: vi.fn(() => runInit('encoderPanel')),
}));

vi.mock('../outputPanel', () => ({
	initOutputPanel: vi.fn(() => runInit('outputPanel')),
}));

vi.mock('../statusPanel/index', () => ({
	initStatusPanel: vi.fn(() => runInit('statusPanel')),
	pushStatusPanelTransientStatus: vi.fn(),
}));

vi.mock('../coverArt', () => ({
	initCoverArt: vi.fn(() => runInit('coverArt')),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
	onClearCoverArt: vi.fn(),
}));

vi.mock('../metadataForm', () => ({
	initMetadataFormEvents: vi.fn(() => runInit('metadataForm')),
	onMetadataFormActionSelectChange: vi.fn(),
	onMetadataFormFieldInput: vi.fn(),
	resetDirtyState: vi.fn(),
	setMetadataFormSaveHandler: vi.fn(),
	triggerMetadataFormSave: vi.fn(),
}));

vi.mock('../tagPreview', () => ({
	initTagPreview: vi.fn(() => runInit('tagPreview')),
}));

vi.mock('../metadataLookup', () => ({
	initMetadataLookup: vi.fn(() => runInit('metadataLookup')),
}));

vi.mock('../jobControls', () => ({
	initJobControls: vi.fn(() => runInit('jobControls')),
	handleMergeModeChange: vi.fn(),
	handleMaxConcurrentSelectionChange: vi.fn(),
}));

describe('App initialization regression guards', () => {
	beforeEach(() => {
		vi.resetModules();
		context.initOrder = [];
		context.failInitAt = null;
		document.body.innerHTML = '<div id="app"></div>';
	});

	it('runs UI init modules in canonical order', async () => {
		await import('../../main');

		await vi.waitFor(() => {
			expect(context.initOrder.length).toBe(9);
		});

		expect(context.initOrder).toEqual([
			'fileImport',
			'encoderPanel',
			'outputPanel',
			'statusPanel',
			'coverArt',
			'metadataForm',
			'tagPreview',
			'metadataLookup',
			'jobControls',
		]);
	});

	it('shows fatal initialization banner and stops later init calls on failure', async () => {
		context.failInitAt = 'metadataLookup';

		await import('../../main');

		await vi.waitFor(() => {
			const alert = document.querySelector('[role="alert"]');
			expect(alert).toBeTruthy();
			expect(alert?.textContent).toContain('Initialization Error');
		});

		expect(context.initOrder).toEqual([
			'fileImport',
			'encoderPanel',
			'outputPanel',
			'statusPanel',
			'coverArt',
			'metadataForm',
			'tagPreview',
			'metadataLookup',
		]);
		expect(context.initOrder).not.toContain('jobControls');
		expect(document.body.textContent).toContain('metadataLookup init failed');
	});
});
