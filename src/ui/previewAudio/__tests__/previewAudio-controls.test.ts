import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import PreviewAudioControls from '../PreviewAudioControls.svelte';

const { triggerProcessFromStatusPanelMock } = vi.hoisted(() => ({
	triggerProcessFromStatusPanelMock: vi.fn(),
}));

vi.mock('../../statusPanel', () => ({
	triggerProcessFromStatusPanel: triggerProcessFromStatusPanelMock,
}));

describe('PreviewAudioControls', () => {
	beforeEach(() => {
		triggerProcessFromStatusPanelMock.mockReset();
	});

	it('dispatches preview through the status panel strip with the default duration', async () => {
		render(PreviewAudioControls);

		await fireEvent.click(screen.getByRole('button', { name: 'Preview Audio' }));

		expect(triggerProcessFromStatusPanelMock).toHaveBeenCalledWith({ previewSeconds: 30 });
	});

	it('dispatches the selected dropdown duration', async () => {
		render(PreviewAudioControls);

		await fireEvent.click(screen.getByRole('button', { name: '15 seconds' }));

		expect(triggerProcessFromStatusPanelMock).toHaveBeenCalledWith({ previewSeconds: 15 });
	});
});
