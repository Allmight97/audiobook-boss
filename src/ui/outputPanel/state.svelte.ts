import type {
	EncoderSettings,
	ExternalToolchainPreference,
	OutputConfig,
	OutputNamingConfig,
	SampleRateConfig,
} from '../../types/audio';
import { defaultEncoderSettings } from '../../types/audio';

export type OutputNamingPreset = OutputNamingConfig['preset'];
const DEFAULT_CUSTOM_TEMPLATE = '{author}/{title}';

export interface OutputPanelState {
	encoderSettings: EncoderSettings;
	toolchainSettings: ExternalToolchainPreference;
	sampleRate: SampleRateConfig;
	outputDirectory: string;
	namingPreset: OutputNamingPreset;
	namingTemplate: string;
	absIncludeYear: boolean;
	previewText: string;
	previewTitle: string;
	absHintText: string;
	absHintHidden: boolean;
	templateRowHidden: boolean;
	estimatedSizeText: string;
}

export const outputPanelState = $state<OutputPanelState>({
	encoderSettings: { ...defaultEncoderSettings() },
	toolchainSettings: {},
	sampleRate: { explicit: 22050 },
	outputDirectory: '',
	namingPreset: 'absDefault',
	namingTemplate: '',
	absIncludeYear: false,
	previewText: 'Select output directory...',
	previewTitle: 'No directory selected',
	absHintText: 'Creates Author / Series / Book # - Title',
	absHintHidden: false,
	templateRowHidden: true,
	estimatedSizeText: '~ --- MB',
});

type OutputPanelRuntimeState = {
	latestPreviewRequestId: number;
};

export const outputPanelRuntimeState = $state<OutputPanelRuntimeState>({
	latestPreviewRequestId: 0,
});

export function beginOutputPreviewRequest(): number {
	outputPanelRuntimeState.latestPreviewRequestId += 1;
	return outputPanelRuntimeState.latestPreviewRequestId;
}

export function isLatestOutputPreviewRequest(requestId: number): boolean {
	return outputPanelRuntimeState.latestPreviewRequestId === requestId;
}

export function getState(): OutputPanelState {
	return outputPanelState;
}

export function getOutputNamingConfig(): OutputNamingConfig {
	const trimmedTemplate = outputPanelState.namingTemplate.trim();
	return {
		preset: outputPanelState.namingPreset,
		includeYear: outputPanelState.absIncludeYear,
		customTemplate:
			outputPanelState.namingPreset === 'customTemplate'
				? trimmedTemplate.length > 0
					? outputPanelState.namingTemplate
					: DEFAULT_CUSTOM_TEMPLATE
				: undefined,
	};
}

export function readOutputConfigForProcessing(): OutputConfig {
	if (!outputPanelState.outputDirectory) {
		throw new Error('Output directory not selected');
	}

	return {
		encoderSettings: outputPanelState.encoderSettings,
		toolchainSettings: outputPanelState.toolchainSettings,
		sampleRate: outputPanelState.sampleRate,
		outputPath: outputPanelState.outputDirectory,
		outputNaming: getOutputNamingConfig(),
	};
}

export function updateOutputDirectory(path: string): void {
	outputPanelState.outputDirectory = path;
}

export function updateNamingPreset(preset: OutputNamingPreset): void {
	outputPanelState.namingPreset = preset;
}

export function updateAbsCompatible(enabled: boolean): void {
	updateNamingPreset(enabled ? 'absDefault' : 'customTemplate');
}

export function updateNamingTemplate(template: string): void {
	outputPanelState.namingTemplate = template;
}

export function updateAbsIncludeYear(enabled: boolean): void {
	outputPanelState.absIncludeYear = enabled;
}

export function updateEncoderSettings(settings: EncoderSettings): void {
	outputPanelState.encoderSettings = settings;
}

export function updateToolchainSettings(settings: ExternalToolchainPreference): void {
	outputPanelState.toolchainSettings = settings;
}

export function updateSampleRate(sampleRate: SampleRateConfig): void {
	outputPanelState.sampleRate = sampleRate;
}

export function setOutputPreview(text: string, title: string = text): void {
	outputPanelState.previewText = text;
	outputPanelState.previewTitle = title;
}

export function setOutputNamingUiState(options: {
	absHintText: string;
	absHintHidden: boolean;
	templateRowHidden: boolean;
}): void {
	outputPanelState.absHintText = options.absHintText;
	outputPanelState.absHintHidden = options.absHintHidden;
	outputPanelState.templateRowHidden = options.templateRowHidden;
}

export function setEstimatedSizeText(value: string): void {
	outputPanelState.estimatedSizeText = value;
}

export function loadInitialState(): void {
	// No-op: outputPanelState is already the canonical state surface.
}
