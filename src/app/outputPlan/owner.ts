import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, CollisionPolicy } from '../../types/audio';
import { formatFileSize } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import type { OutputPreview } from '../../types/session';
import type { EngineLink } from '../engineLink';
import { collisionView, type CollisionView } from './collision';
import {
	EMPTY_PREVIEW_TEXT,
	EMPTY_PREVIEW_TITLE,
	namingHintText,
	PREVIEW_UNAVAILABLE_TEXT,
	type OutputView,
} from './types';

/**
 * The engine owns where exports go, how they are named, the path preview,
 * and each title's size estimate. This owner shows them, opens the folder
 * picker, and holds the collision dialog a submission asks through.
 */
export type OutputPlanOwner = {
	readonly view: Accessor<OutputView>;
	estimateTitleSizeText(file: AudioFile): string | null;
	readonly collision: Accessor<CollisionView>;
	browseDirectory(): Promise<void>;
	selectNamingPreset(value: string): void;
	setAbsIncludeYear(value: boolean): void;
	editNamingTemplate(value: string): void;
	chooseCollisionPolicy(reviewId: number, policy: CollisionPolicy): void;
	cancelCollisionReview(reviewId: number): void;
	reset(): void;
};

export type OutputOwnerDeps = {
	readonly link: EngineLink;
};

function previewText(preview: OutputPreview, directory: string | null): string {
	switch (preview.kind) {
		case 'noDirectory':
			return EMPTY_PREVIEW_TEXT;
		case 'noTitle':
			return directory ?? EMPTY_PREVIEW_TEXT;
		case 'path':
			return preview.path;
		case 'unavailable':
			return PREVIEW_UNAVAILABLE_TEXT;
	}
}

export function createOutputOwner(deps: OutputOwnerDeps): OutputPlanOwner {
	const { link } = deps;
	let disposed = false;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	// The template as typed, shown until the engine confirms it.
	let typedTemplate: { readonly value: string } | undefined;

	const view: Accessor<OutputView> = () => {
		rev();
		const output = link.output();
		const text = previewText(output.preview, output.directory);
		return {
			outputDirectory: output.directory ?? '',
			namingPreset: output.preset,
			namingTemplate: typedTemplate?.value ?? output.template,
			absIncludeYear: output.includeYear,
			previewText: text,
			previewTitle: output.preview.kind === 'noDirectory' ? EMPTY_PREVIEW_TITLE : text,
			absHintText: namingHintText(output.preset, output.includeYear),
			absHintHidden: output.preset !== 'absDefault',
			templateRowHidden: output.preset !== 'customTemplate',
			displayDirectory: output.directory || EMPTY_PREVIEW_TEXT,
		};
	};

	return {
		view,
		estimateTitleSizeText(file) {
			if (!file.isValid) return null;
			const estimate = link.audio().titles[file.inputId ?? file.path]?.estimate;
			if (!estimate) return null;
			return estimate.kind === 'bytes'
				? `Est. ~ ${formatFileSize(estimate.bytes)}`
				: 'Size varies with audio';
		},
		collision: () => {
			rev();
			return collisionView(disposed ? null : link.output().collisionReview);
		},
		async browseDirectory() {
			try {
				const directory = await tauriClient.openDirectory({ title: 'Select Output Directory' });
				if (directory) link.post({ kind: 'setOutputDirectory', directory });
			} catch (cause) {
				console.error('Error selecting directory:', cause);
			}
		},
		selectNamingPreset(value) {
			const preset = value === 'customTemplate' ? 'customTemplate' : 'absDefault';
			link.post({ kind: 'setNamingPreset', preset });
		},
		setAbsIncludeYear(includeYear) {
			link.post({ kind: 'setIncludeYear', includeYear });
		},
		editNamingTemplate(template) {
			const entry = { value: template };
			typedTemplate = entry;
			bump((n) => n + 1);
			link
				.send({ kind: 'setNamingTemplate', template })
				.catch((error: unknown) => console.error('Failed to record the naming template:', error))
				.finally(() => {
					if (typedTemplate !== entry) return;
					typedTemplate = undefined;
					bump((n) => n + 1);
				});
		},
		chooseCollisionPolicy(reviewId, policy) {
			if (!disposed) link.post({ kind: 'chooseCollisionPolicy', reviewId, policy });
		},
		cancelCollisionReview(reviewId) {
			if (!disposed) link.post({ kind: 'cancelCollisionReview', reviewId });
		},
		reset() {
			disposed = true;
			typedTemplate = undefined;
			bump((n) => n + 1);
		},
	};
}
