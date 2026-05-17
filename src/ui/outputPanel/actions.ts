import { tauriClient } from '../../lib/tauri/client';
import {
	updateOutputDirectory,
	updateNamingPreset,
	updateNamingTemplate,
	updateAbsIncludeYear,
	type OutputNamingPreset,
} from './state.svelte';
import { updateOutputPath, updateNamingOptionState, showOutputError } from './preview';

const TEMPLATE_PREVIEW_DEBOUNCE_MS = 150;
let templatePreviewDebounceTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleTemplatePreviewUpdate(): void {
	if (templatePreviewDebounceTimer) {
		clearTimeout(templatePreviewDebounceTimer);
	}
	templatePreviewDebounceTimer = setTimeout(() => {
		templatePreviewDebounceTimer = null;
		updateOutputPath('final');
	}, TEMPLATE_PREVIEW_DEBOUNCE_MS);
}

function resetTemplatePreviewDebounce(): void {
	if (templatePreviewDebounceTimer) {
		clearTimeout(templatePreviewDebounceTimer);
		templatePreviewDebounceTimer = null;
	}
}

export async function browseOutputDirectory(): Promise<void> {
	try {
		const selectedPath = await tauriClient.openDirectory({
			title: 'Select Output Directory',
		});

		if (selectedPath) {
			updateOutputDirectory(selectedPath);
			updateOutputPath('final');
		}
	} catch (error) {
		console.error('Error selecting directory:', error);
		showOutputError('Failed to select directory');
	}
}

export function selectNamingPreset(value: string): void {
	const preset: OutputNamingPreset = value === 'customTemplate' ? 'customTemplate' : 'absDefault';
	updateNamingPreset(preset);
	updateNamingOptionState();
	updateOutputPath('final');
}

export function setAbsIncludeYear(checked: boolean): void {
	updateAbsIncludeYear(checked);
	updateNamingOptionState();
	updateOutputPath('final');
}

export function editNamingTemplate(value: string): void {
	updateNamingTemplate(value);
	scheduleTemplatePreviewUpdate();
}

export function resetOutputPanelActions(): void {
	resetTemplatePreviewDebounce();
}
