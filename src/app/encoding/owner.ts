import { createSignal, type Accessor } from 'solid-js';
import type { EncoderDefaults } from '../../types/appSettings';
import type { AudioFile, TitleAudioRequest, EncoderSettingsCapabilities } from '../../types/audio';
import type { InputOwner } from '../inputSession';
import { resolveAutoResolutionHints } from './hints';
import {
	applyCapabilities,
	applyDefaultsToBag,
	bagDefaults,
	bagEstimateKbps,
	bagRequest,
	createDefaultBag,
	projectView,
	selectField,
	syncPolicy,
	type EncodingBag,
	type EncodingField,
	type EncodingView,
} from './project';

export type { EncodingField, EncodingView } from './project';

export type EncodingOwner = {
	audioRequest(file?: AudioFile): TitleAudioRequest;
	titleView(file: AudioFile): EncodingView;
	selectionView(
		files: readonly AudioFile[],
	): EncodingView & { mixedFields: readonly EncodingField[] };
	selectTitles(files: readonly AudioFile[], field: EncodingField, value: string): void;
	applyDefaultsToTitles(files: readonly AudioFile[]): void;
	estimateTitleKbps(file: AudioFile): number | null;
	selectTitle(file: AudioFile, field: EncodingField, value: string): void;
	readonly view: Accessor<EncodingView>;
	select(field: EncodingField, value: string): void;
	setAfterburner(enabled: boolean): void;
	applyDefaults(defaults: EncoderDefaults): void;
	hydrateDefaults(defaults: EncoderDefaults): void;
	readDefaults(): EncoderDefaults;
	reloadCapabilities(capabilities?: EncoderSettingsCapabilities | null): Promise<void>;
	reset(): void;
};

export type EncodingOwnerDeps = {
	readonly input: Pick<InputOwner, 'view' | 'audioRequest' | 'setAudioRequest' | 'sourcesFor'>;
	readonly loadCapabilities: () => Promise<EncoderSettingsCapabilities | null>;
	readonly onFdkSetupRequested?: () => void;
	readonly persistDefaults?: (defaults: EncoderDefaults) => void;
};

const fieldKeys = {
	format: 'format',
	intent: 'intent',
	encoder: 'flavor',
	quality: 'quality',
	faacProfile: 'faacProfile',
	rateControl: 'rateControl',
	nativeSpeed: 'nativeSpeed',
	bitrate: 'bitrate',
	sampleRate: 'sampleRate',
	channels: 'channels',
} as const satisfies Record<EncodingField, keyof EncodingView>;

export function createEncodingOwner(deps: EncodingOwnerDeps): EncodingOwner {
	let bag: EncodingBag = createDefaultBag();
	let generation = 0;
	let defaultsEdited = false;
	const [rev, bump] = createSignal(0, { ownedWrite: true });

	function publish(): void {
		bump((n) => n + 1);
	}

	function commitPolicy(): ReturnType<typeof syncPolicy> {
		const result = syncPolicy(bag);
		publish();
		return result;
	}

	function persist(): void {
		deps.persistDefaults?.(bagDefaults(bag));
	}

	async function loadCapabilities(supplied?: EncoderSettingsCapabilities | null): Promise<void> {
		const ticket = ++generation;
		try {
			const capabilities = supplied === undefined ? await deps.loadCapabilities() : supplied;
			if (ticket !== generation) return;
			applyCapabilities(bag, capabilities);
			syncPolicy(bag);
			publish();
		} catch (error) {
			if (ticket !== generation) return;
			console.warn('Failed to load encoder capabilities:', error);
			publish();
		}
	}

	void loadCapabilities();

	function requestFromBag(current: EncodingBag): TitleAudioRequest {
		const request = bagRequest(current);
		return {
			format: current.format,
			intent: current.intent,
			settings: current.format === 'mp3' ? null : request.encoderSettings,
			sampleRate: request.sampleRate,
		};
	}
	function titleBag(file: AudioFile): EncodingBag {
		rev();
		const override = deps.input.audioRequest(file);
		const current = { ...bag };
		if (override?.settings)
			applyDefaultsToBag(current, { ...override, settings: override.settings });
		current.format = override?.format ?? current.format;
		current.intent = override?.intent ?? current.intent;
		if (override)
			current.sampleRate =
				override.sampleRate === 'auto' ? 'auto' : String(override.sampleRate.explicit);
		const hints = resolveAutoResolutionHints(deps.input.sourcesFor(file));
		current.sampleRateHint = hints.sampleRateHint;
		current.channelsHint = hints.channelsHint;
		current.hasMultichannelInput = hints.hasMultichannelInput;
		return current;
	}
	function selectTitles(files: readonly AudioFile[], field: EncodingField, value: string): void {
		if (deps.input.view().orderLocked) return;
		if (
			field === 'encoder' &&
			value === 'fdk_he_aac' &&
			bag.availability &&
			!bag.availability.fdkAvailable
		) {
			deps.onFdkSetupRequested?.();
			return;
		}
		for (const file of files) {
			const current = titleBag(file);
			const previous = String(projectView(current)[fieldKeys[field]]);
			if (!selectField(current, field, value) && previous !== value) continue;
			if (field !== 'format' && field !== 'intent') current.intent = 'encode';
			deps.input.setAudioRequest(file, requestFromBag(current));
		}
	}

	return {
		audioRequest(file) {
			rev();
			return file ? (deps.input.audioRequest(file) ?? requestFromBag(bag)) : requestFromBag(bag);
		},
		estimateTitleKbps(file) {
			return bagEstimateKbps(titleBag(file));
		},
		titleView(file) {
			return projectView(titleBag(file));
		},
		selectTitle(file, field, value) {
			selectTitles([file], field, value);
		},
		selectTitles,
		selectionView(files) {
			const views = files.map((file) => projectView(titleBag(file)));
			const combined = files[0] ? titleBag(files[0]) : { ...bag };
			Object.assign(
				combined,
				resolveAutoResolutionHints(files.flatMap((file) => [...deps.input.sourcesFor(file)])),
			);
			const first = projectView(combined);
			const mixedFields = (Object.keys(fieldKeys) as EncodingField[]).filter((field) =>
				views.some((view) => view[fieldKeys[field]] !== first[fieldKeys[field]]),
			);
			return { ...first, mixedFields };
		},
		applyDefaultsToTitles(files) {
			for (const file of files) deps.input.setAudioRequest(file, requestFromBag(bag));
		},
		view: () => {
			rev();
			return projectView(bag);
		},
		select(field, value) {
			if (
				field === 'encoder' &&
				value === 'fdk_he_aac' &&
				bag.availability &&
				!bag.availability.fdkAvailable
			) {
				deps.onFdkSetupRequested?.();
				return;
			}
			if (!selectField(bag, field, value)) return;
			defaultsEdited = true;
			const { flavorReset } = commitPolicy();
			if (flavorReset) return;
			persist();
		},
		setAfterburner(enabled) {
			if (bag.afterburner === enabled) return;
			defaultsEdited = true;
			bag.afterburner = enabled;
			commitPolicy();
			persist();
		},
		hydrateDefaults(defaults) {
			if (defaultsEdited) return;
			applyDefaultsToBag(bag, defaults);
			commitPolicy();
		},
		applyDefaults(defaults) {
			defaultsEdited = true;
			applyDefaultsToBag(bag, defaults);
			commitPolicy();
		},
		readDefaults() {
			return bagDefaults(bag);
		},
		reloadCapabilities(capabilities) {
			return loadCapabilities(capabilities);
		},
		reset() {
			generation += 1;
			defaultsEdited = true;
			bag = createDefaultBag();
			publish();
		},
	};
}
