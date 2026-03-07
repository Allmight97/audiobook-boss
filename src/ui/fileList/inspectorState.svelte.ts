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
}
