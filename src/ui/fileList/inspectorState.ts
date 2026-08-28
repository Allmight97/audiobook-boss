import { Atom } from '../../lib/effect/appEffect';
import { fileListAtomRegistry } from './state';

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

export const inspectorAtom = Atom.make<InspectorState>({ ...EMPTY_INSPECTOR_STATE }).pipe(
	Atom.keepAlive,
);

export function getInspectorState(): InspectorState {
	return fileListAtomRegistry.get(inspectorAtom);
}

export const inspectorState: InspectorState = new Proxy({} as InspectorState, {
	get(_target, property) {
		return getInspectorState()[property as keyof InspectorState];
	},
});

export function setInspectorContext(options: {
	text: string;
	variant: InspectorState['contextVariant'];
	detail?: string;
}): void {
	fileListAtomRegistry.update(inspectorAtom, (state) => ({
		...state,
		contextText: options.text,
		contextVariant: options.variant,
		contextDetail: options.detail ?? '',
	}));
}

export function setInspectorValues(options: {
	bitrateText: string;
	sampleRateText: string;
	channelsText: string;
	codecText: string;
	decoderText: string;
	fileSizeText: string;
}): void {
	fileListAtomRegistry.update(inspectorAtom, (state) => ({
		...state,
		bitrateText: options.bitrateText,
		sampleRateText: options.sampleRateText,
		channelsText: options.channelsText,
		codecText: options.codecText,
		decoderText: options.decoderText,
		fileSizeText: options.fileSizeText,
	}));
}

export function setInspectorCompanions(options: { text: string; title: string }): void {
	fileListAtomRegistry.update(inspectorAtom, (state) => ({
		...state,
		companionsText: options.text,
		companionsTitle: options.title,
	}));
}

export function resetInspectorState(): void {
	fileListAtomRegistry.set(inspectorAtom, { ...EMPTY_INSPECTOR_STATE });
}
