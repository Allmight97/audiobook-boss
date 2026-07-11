import { describe, expect, it } from 'vitest';
import appSource from '../../App.svelte?raw';
import fileInspectorSource from '../leftColumn/FileInspectorPanel.svelte?raw';
import inputWorkflowSource from '../leftColumn/InputWorkflowPanel.svelte?raw';
import leftColumnSource from '../leftColumn/LeftColumnIsland.svelte?raw';

describe('left column sibling layout', () => {
	it('keeps App as shell composition for the left column', () => {
		expect(appSource).toContain('<LeftColumnIsland />');
		expect(appSource).not.toContain('<FileImportIsland');
		expect(appSource).not.toContain('<JobControlsIsland');
		expect(appSource).not.toContain('inspectorState');
		expect(appSource).not.toContain('Selected File Properties');
	});

	it('renders input workflow before file inspector as sibling zones', () => {
		const inputIndex = leftColumnSource.indexOf('<InputWorkflowPanel />');
		const inspectorIndex = leftColumnSource.indexOf('<FileInspectorPanel />');

		expect(leftColumnSource).toContain('data-testid="left-column"');
		expect(inputIndex).toBeGreaterThan(-1);
		expect(inspectorIndex).toBeGreaterThan(inputIndex);
	});

	it('keeps input workflow composition in existing behavior owners', () => {
		expect(inputWorkflowSource).toContain('Input and File Order');
		expect(inputWorkflowSource).toContain('<FileImportIsland');
		expect(inputWorkflowSource).not.toContain('<JobControlsIsland');
	});

	it('keeps file inspector bound to existing FileList inspector state', () => {
		for (const label of [
			'Bitrate:',
			'Sample Rate:',
			'Channels:',
			'Codec:',
			'Decoder:',
			'File Size:',
			'Supplemental:',
			'Combined Size:',
		]) {
			expect(fileInspectorSource).toContain(label);
		}

		expect(fileInspectorSource).toContain('inspectorState');
		expect(fileInspectorSource).toContain('readCombinedSizeText');
		expect(fileInspectorSource).toContain('data-testid="file-inspector-panel"');
	});
});
