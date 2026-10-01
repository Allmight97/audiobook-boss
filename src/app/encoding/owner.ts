import type { Accessor } from 'solid-js';
import type { AudioFile, TitleAudioPlan, TitleAudioRequest } from '../../types/audio';
import type { AudioChoiceView, TitlePlan } from '../../types/session';
import type { EngineLink } from '../engineLink';
import type { InputOwner } from '../inputSession';
import { estimateKbpsFromSettings } from './estimate';
import { resolveAutoResolutionHints, type AutoResolutionHints } from './hints';
import { editFor, projectView, type EncodingField, type EncodingView } from './project';

export type { EncodingField, EncodingView } from './project';

/**
 * The engine owns the audio choices: the defaults new titles start from and
 * each title's own choice, with the rules for editing them. This owner shows
 * them in the encoder panel's terms and sends edits.
 */
export type EncodingOwner = {
	/** The request processing receives for `file`, or the defaults' request. */
	audioRequest(file?: AudioFile): TitleAudioRequest;
	/** What the engine resolved `file`'s audio to. */
	plan(file: AudioFile): TitlePlan;
	titleView(file: AudioFile): EncodingView;
	selectionView(
		files: readonly AudioFile[],
	): EncodingView & { mixedFields: readonly EncodingField[] };
	selectTitles(files: readonly AudioFile[], field: EncodingField, value: string): void;
	applyDefaultsToTitles(files: readonly AudioFile[]): void;
	estimateTitleKbps(file: AudioFile, plan?: TitleAudioPlan): number | null;
	selectTitle(file: AudioFile, field: EncodingField, value: string): void;
	/** The defaults, as the Settings panel edits them. */
	readonly view: Accessor<EncodingView>;
	/** Changes when encoder capabilities arrive. */
	readonly capabilityRevision: Accessor<number>;
	select(field: EncodingField, value: string): void;
	reset(): void;
};

export type EncodingOwnerDeps = {
	readonly link: EngineLink;
	readonly input: Pick<InputOwner, 'sourcesFor'>;
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

// Defaults describe future imports, so their hints name no source.
const DEFAULT_HINTS = resolveAutoResolutionHints([]);

function titleId(file: AudioFile): string {
	return file.inputId ?? file.path;
}

export function createEncodingOwner(deps: EncodingOwnerDeps): EncodingOwner {
	const { link } = deps;

	function titleChoice(file: AudioFile): AudioChoiceView {
		const audio = link.audio();
		return audio.titles[titleId(file)] ?? audio.defaults;
	}

	function display(view: AudioChoiceView, hints: AutoResolutionHints): EncodingView {
		return projectView({
			choice: view.choice,
			facts: view.facts,
			capabilities: link.audio().capabilities,
			hints,
		});
	}

	function titleView(file: AudioFile): EncodingView {
		return display(titleChoice(file), resolveAutoResolutionHints(deps.input.sourcesFor(file)));
	}

	function selectTitles(files: readonly AudioFile[], field: EncodingField, value: string): void {
		const edit = editFor(field, value);
		if (!edit || files.length === 0) return;
		link.post({ kind: 'setTitleAudio', titleIds: files.map(titleId), edit });
	}

	return {
		audioRequest(file) {
			return file ? titleChoice(file).request : link.audio().defaults.request;
		},
		plan(file) {
			return link.audio().titles[titleId(file)]?.plan ?? { kind: 'pending' };
		},
		estimateTitleKbps(file, plan) {
			if (plan?.settings) return estimateKbpsFromSettings(plan.settings);
			return titleChoice(file).facts.estimateKbps;
		},
		titleView,
		selectTitle(file, field, value) {
			selectTitles([file], field, value);
		},
		selectTitles,
		selectionView(files) {
			const views = files.map(titleView);
			const hints = resolveAutoResolutionHints(
				files.flatMap((file) => [...deps.input.sourcesFor(file)]),
			);
			const first = files[0]
				? display(titleChoice(files[0]), hints)
				: display(link.audio().defaults, hints);
			const mixedFields = (Object.keys(fieldKeys) as EncodingField[]).filter((field) =>
				views.some((view) => view[fieldKeys[field]] !== first[fieldKeys[field]]),
			);
			return { ...first, mixedFields };
		},
		applyDefaultsToTitles(files) {
			if (files.length === 0) return;
			link.post({ kind: 'applyDefaultAudio', titleIds: files.map(titleId) });
		},
		view: () => display(link.audio().defaults, DEFAULT_HINTS),
		capabilityRevision: () => (link.audio().capabilities ? 1 : 0),
		select(field, value) {
			const edit = editFor(field, value);
			if (edit) link.post({ kind: 'setDefaultAudio', edit });
		},
		reset() {},
	};
}
