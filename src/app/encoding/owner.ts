import type { Accessor } from 'solid-js';
import type { AudioFile, TitleAudioRequest } from '../../types/audio';
import type { AudioChoiceView, TitlePlan } from '../../types/session';
import type { EngineLink } from '../engineLink';
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
	/** The selected titles' audio as the engine combines it; `null` with
	 * nothing selected. */
	selectionView(
		files: readonly AudioFile[],
	): (EncodingView & { mixedFields: readonly EncodingField[] }) | null;
	/** Why the latest edit of `files`' audio changed nothing, worded. */
	refusal(files: readonly AudioFile[]): string | null;
	selectTitles(files: readonly AudioFile[], field: EncodingField, value: string): void;
	applyDefaultsToTitles(files: readonly AudioFile[]): void;
	selectTitle(file: AudioFile, field: EncodingField, value: string): void;
	/** The defaults, as the Settings panel edits them. */
	readonly view: Accessor<EncodingView>;
	select(field: EncodingField, value: string): void;
	reset(): void;
};

export type EncodingOwnerDeps = {
	readonly link: EngineLink;
};

function titleId(file: AudioFile): string {
	return file.inputId ?? file.path;
}

export function createEncodingOwner(deps: EncodingOwnerDeps): EncodingOwner {
	const { link } = deps;

	function titleChoice(file: AudioFile): AudioChoiceView {
		const audio = link.audio();
		return audio.titles[titleId(file)] ?? audio.defaults;
	}

	function plan(file: AudioFile): TitlePlan {
		return link.audio().titles[titleId(file)]?.plan ?? { kind: 'pending' };
	}

	/** `titles` is empty for the defaults, which describe future imports. */
	function display(
		view: Pick<AudioChoiceView, 'choice' | 'facts'>,
		titles: readonly AudioFile[],
	): EncodingView {
		return projectView({
			choice: view.choice,
			facts: view.facts,
			capabilities: link.audio().capabilities,
			plans: titles.length === 0 ? null : titles.map(plan),
		});
	}

	function titleView(file: AudioFile): EncodingView {
		return display(titleChoice(file), [file]);
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
		plan,
		titleView,
		selectTitle(file, field, value) {
			selectTitles([file], field, value);
		},
		selectTitles,
		selectionView(files) {
			const selection = link.audio().selection;
			if (!selection) return null;
			return {
				...display(selection, files),
				mixedFields: selection.mixed,
			};
		},
		refusal(files) {
			const refusal = link.audio().refusal;
			const shown = new Set(files.map(titleId));
			if (!refusal) return null;
			if (refusal.kind === 'locked') {
				return "Audio can't change while an export is being prepared.";
			}
			const names = refusal.labels.filter((_, index) => shown.has(refusal.titleIds[index] ?? ''));
			if (names.length === 0) return null;
			return `${names.join(', ')} can't take that change, so no title changed.`;
		},
		applyDefaultsToTitles(files) {
			if (files.length === 0) return;
			link.post({ kind: 'applyDefaultAudio', titleIds: files.map(titleId) });
		},
		view: () => display(link.audio().defaults, []),
		select(field, value) {
			const edit = editFor(field, value);
			if (edit) link.post({ kind: 'setDefaultAudio', edit });
		},
		reset() {},
	};
}
