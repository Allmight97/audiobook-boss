import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, CollisionPolicy } from '../../types/audio';
import { formatFileSize } from '../../types/audio';
import { logFrontendFailure } from '../../lib/frontendLogBridge';
import { tauriClient } from '../../lib/tauri/client';
import type { NamingPreset } from '../../lib/generated/tauri';
import type { OutputPreview } from '../../types/session';
import { createTypingEcho, type EngineLink } from '../engineLink';
import { collisionView, type CollisionView } from './collision';
import { EMPTY_PREVIEW_TEXT, EMPTY_PREVIEW_TITLE, type OutputView } from './types';

export type OutputPlanOwner = {
	readonly view: Accessor<OutputView>;
	estimateTitleSizeText: (file: AudioFile) => string | null;
	readonly collision: Accessor<CollisionView>;
	browseDirectory: () => Promise<void>;
	selectNamingPreset: (value: string) => void;
	setAbsIncludeYear: (value: boolean) => void;
	editNamingTemplate: (value: string) => void;
	chooseCollisionPolicy: (reviewId: number, policy: CollisionPolicy) => void;
	cancelCollisionReview: (reviewId: number) => void;
	reset: () => void;
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
			return preview.message;
	}
}

export function createOutputOwner(deps: OutputOwnerDeps): OutputPlanOwner {
	const { link } = deps;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const typed = createTypingEcho<'template'>({
		scopeOf: () => [],
		changed: () => bump((n) => n + 1),
	});

	const view: Accessor<OutputView> = () => {
		rev();
		const output = link.output();
		const text = previewText(output.preview, output.directory);
		return {
			outputDirectory: output.directory ?? '',
			namingPreset: output.preset,
			namingTemplate: typed.valueOf('template') ?? output.template,
			absIncludeYear: output.includeYear,
			previewText: text,
			previewTitle: output.preview.kind === 'noDirectory' ? EMPTY_PREVIEW_TITLE : text,
			templateRowHidden: output.preset !== 'customTemplate',
			displayDirectory: output.directory || EMPTY_PREVIEW_TEXT,
		};
	};

	return {
		view,
		estimateTitleSizeText(file) {
			const estimate = link.audio().titles[file.inputId]?.estimate;
			if (!estimate) return null;
			return estimate.kind === 'bytes'
				? `Est. ~ ${formatFileSize(estimate.bytes)}`
				: 'Size varies with audio';
		},
		collision: () => collisionView(link.output().collisionReview),
		async browseDirectory() {
			try {
				const directory = await tauriClient.openDirectory({ title: 'Select Output Directory' });
				if (directory) link.post({ kind: 'setOutputDirectory', directory });
			} catch (cause) {
				logFrontendFailure('outputPlan.selectDirectory', cause);
			}
		},
		selectNamingPreset(value) {
			// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the select offers only NamingPreset values.
			link.post({ kind: 'setNamingPreset', preset: value as NamingPreset });
		},
		setAbsIncludeYear(includeYear) {
			link.post({ kind: 'setIncludeYear', includeYear });
		},
		editNamingTemplate(template) {
			typed.show('template', template, () =>
				link
					.send({ kind: 'setNamingTemplate', template })
					.catch((error: unknown) => logFrontendFailure('outputPlan.setNamingTemplate', error)),
			);
		},
		chooseCollisionPolicy(reviewId, policy) {
			link.post({ kind: 'chooseCollisionPolicy', reviewId, policy });
		},
		cancelCollisionReview(reviewId) {
			link.post({ kind: 'cancelCollisionReview', reviewId });
		},
		reset() {
			typed.clear();
		},
	};
}
