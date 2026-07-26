import { readCombinedSizeText } from './viewState.svelte';

type InspectorState = {
	contextText: string;
	contextVariant: 'empty' | 'single' | 'multi';
	contextDetail: string;
	bitrateText: string;
	sampleRateText: string;
	channelsText: string;
	codecText: string;
	decoderText: string;
	fileSizeText: string;
	companionsText: string;
	companionsTitle: string;
};

export type InspectorFact = {
	label: string;
	value: string;
	title: string;
};

const EMPTY_INSPECTOR_STATE: InspectorState = {
	contextText: 'No file selected',
	contextVariant: 'empty',
	contextDetail: '',
	bitrateText: '---',
	sampleRateText: '---',
	channelsText: '---',
	codecText: '---',
	decoderText: '---',
	fileSizeText: '---',
	companionsText: '---',
	companionsTitle: '',
};

export const inspectorState = $state<InspectorState>({ ...EMPTY_INSPECTOR_STATE });

export function setInspectorContext(options: {
	text: string;
	variant: InspectorState['contextVariant'];
	detail?: string;
}): void {
	inspectorState.contextText = options.text;
	inspectorState.contextVariant = options.variant;
	inspectorState.contextDetail = options.detail ?? '';
}

export function setInspectorValues(options: {
	bitrateText: string;
	sampleRateText: string;
	channelsText: string;
	codecText: string;
	decoderText: string;
	fileSizeText: string;
}): void {
	inspectorState.bitrateText = options.bitrateText;
	inspectorState.sampleRateText = options.sampleRateText;
	inspectorState.channelsText = options.channelsText;
	inspectorState.codecText = options.codecText;
	inspectorState.decoderText = options.decoderText;
	inspectorState.fileSizeText = options.fileSizeText;
}

export function setInspectorCompanions(options: { text: string; title: string }): void {
	inspectorState.companionsText = options.text;
	inspectorState.companionsTitle = options.title;
}

export function resetInspectorState(): void {
	inspectorState.contextText = EMPTY_INSPECTOR_STATE.contextText;
	inspectorState.contextVariant = EMPTY_INSPECTOR_STATE.contextVariant;
	inspectorState.contextDetail = EMPTY_INSPECTOR_STATE.contextDetail;
	inspectorState.bitrateText = EMPTY_INSPECTOR_STATE.bitrateText;
	inspectorState.sampleRateText = EMPTY_INSPECTOR_STATE.sampleRateText;
	inspectorState.channelsText = EMPTY_INSPECTOR_STATE.channelsText;
	inspectorState.codecText = EMPTY_INSPECTOR_STATE.codecText;
	inspectorState.decoderText = EMPTY_INSPECTOR_STATE.decoderText;
	inspectorState.fileSizeText = EMPTY_INSPECTOR_STATE.fileSizeText;
	inspectorState.companionsText = EMPTY_INSPECTOR_STATE.companionsText;
	inspectorState.companionsTitle = EMPTY_INSPECTOR_STATE.companionsTitle;
}

/** Read-only inspector projection for the metadata surface Facts tab. */
export function readInspectorFacts(): InspectorFact[] {
	return [
		{ label: 'File', value: inspectorState.contextText, title: inspectorState.contextText },
		{
			label: 'Position',
			value: inspectorState.contextDetail || '—',
			title: inspectorState.contextDetail,
		},
		{ label: 'Bitrate', value: inspectorState.bitrateText, title: inspectorState.bitrateText },
		{
			label: 'Sample rate',
			value: inspectorState.sampleRateText,
			title: inspectorState.sampleRateText,
		},
		{ label: 'Channels', value: inspectorState.channelsText, title: inspectorState.channelsText },
		{ label: 'Codec', value: inspectorState.codecText, title: inspectorState.codecText },
		{ label: 'Decoder', value: inspectorState.decoderText, title: inspectorState.decoderText },
		{ label: 'File size', value: inspectorState.fileSizeText, title: inspectorState.fileSizeText },
		{
			label: 'Supplemental',
			value: inspectorState.companionsText,
			title: inspectorState.companionsTitle,
		},
		{ label: 'Combined size', value: readCombinedSizeText(), title: readCombinedSizeText() },
	];
}
