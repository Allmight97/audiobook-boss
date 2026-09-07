import { createEffect, createSignal, type Accessor } from 'solid-js';
import type { EncoderDefaults } from '../../types/appSettings';
import type {
	AudioFile,
	EncoderSettingsCapabilities,
	EncodingRequestConfig,
} from '../../types/audio';
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
	readonly view: Accessor<EncodingView>;
	readonly request: Accessor<EncodingRequestConfig>;
	readonly estimateKbps: Accessor<number>;
	select(field: EncodingField, value: string): void;
	setAfterburner(enabled: boolean): void;
	applyDefaults(defaults: EncoderDefaults): void;
	readDefaults(): EncoderDefaults;
	reloadCapabilities(): Promise<void>;
	reset(): void;
};

export type EncodingOwnerDeps = {
	readonly input: Pick<InputOwner, 'view'>;
	readonly loadCapabilities: () => Promise<EncoderSettingsCapabilities | null>;
	readonly persistDefaults?: (defaults: EncoderDefaults) => void;
};

function selectedFilesFromInput(input: Pick<InputOwner, 'view'>): AudioFile[] {
	const view = input.view();
	return view.selectedIndices
		.map((index) => view.files[index])
		.filter((file): file is AudioFile => Boolean(file));
}

export function createEncodingOwner(deps: EncodingOwnerDeps): EncodingOwner {
	let bag: EncodingBag = createDefaultBag();
	let generation = 0;
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
		try {
			deps.persistDefaults?.(bagDefaults(bag));
		} catch (error) {
			console.warn('Failed to persist encoder defaults:', error);
		}
	}

	async function loadCapabilities(): Promise<void> {
		const ticket = ++generation;
		try {
			const capabilities = await deps.loadCapabilities();
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

	createEffect(
		() => deps.input.view(),
		(inputView) => {
			const selected = inputView.selectedIndices
				.map((index) => inputView.files[index])
				.filter((file): file is AudioFile => Boolean(file));
			const hints = resolveAutoResolutionHints(selected);
			if (bag.sampleRateHint === hints.sampleRateHint && bag.channelsHint === hints.channelsHint) {
				return;
			}
			bag.sampleRateHint = hints.sampleRateHint;
			bag.channelsHint = hints.channelsHint;
			publish();
		},
	);

	return {
		view: () => {
			rev();
			return projectView(bag);
		},
		request: () => {
			rev();
			return bagRequest(bag);
		},
		estimateKbps: () => {
			rev();
			return bagEstimateKbps(bag);
		},
		select(field, value) {
			if (!selectField(bag, field, value)) return;
			const { flavorReset } = commitPolicy();
			if (flavorReset) return;
			persist();
		},
		setAfterburner(enabled) {
			if (bag.afterburner === enabled) return;
			bag.afterburner = enabled;
			commitPolicy();
			persist();
		},
		applyDefaults(defaults) {
			applyDefaultsToBag(bag, defaults);
			commitPolicy();
		},
		readDefaults() {
			return bagDefaults(bag);
		},
		reloadCapabilities() {
			return loadCapabilities();
		},
		reset() {
			generation += 1;
			bag = createDefaultBag();
			const hints = resolveAutoResolutionHints(selectedFilesFromInput(deps.input));
			bag.sampleRateHint = hints.sampleRateHint;
			bag.channelsHint = hints.channelsHint;
			publish();
		},
	};
}
