export type OutputNamingPreset = 'absDefault' | 'customTemplate';

export type OutputDraftMirror = {
	directory: string;
	namingPreset: OutputNamingPreset;
	namingTemplate: string;
	includeYear: boolean;
};

type QueueMirror = {
	summary: string;
	statusText: string;
};

type AppStoreState = {
	selectedIndices: number[];
	pendingMetadataSummary: string;
	outputDraft: OutputDraftMirror;
	queue: QueueMirror;
};

const DEFAULT_OUTPUT_DRAFT: OutputDraftMirror = {
	directory: '',
	namingPreset: 'absDefault',
	namingTemplate: '',
	includeYear: false,
};

export const appStore = $state<AppStoreState>({
	selectedIndices: [],
	pendingMetadataSummary: 'No pending metadata drafts',
	outputDraft: { ...DEFAULT_OUTPUT_DRAFT },
	queue: {
		summary: 'Idle',
		statusText: 'Idle',
	},
});

export function publishSelectedIndices(indices: Iterable<number>): void {
	appStore.selectedIndices = Array.from(indices).sort((a, b) => a - b);
}

export function publishPendingMetadataSummary(pendingCount: number): void {
	appStore.pendingMetadataSummary =
		pendingCount > 0
			? `${pendingCount} pending metadata draft${pendingCount === 1 ? '' : 's'}`
			: 'No pending metadata drafts';
}

export function publishOutputDraft(draft: Partial<OutputDraftMirror>): void {
	appStore.outputDraft = {
		...appStore.outputDraft,
		...draft,
	};
}

export function publishQueueMirror(next: Partial<QueueMirror>): void {
	appStore.queue = {
		...appStore.queue,
		...next,
	};
}

export function resetAppStoreMirrors(): void {
	appStore.selectedIndices = [];
	appStore.pendingMetadataSummary = 'No pending metadata drafts';
	appStore.outputDraft = { ...DEFAULT_OUTPUT_DRAFT };
	appStore.queue = { summary: 'Idle', statusText: 'Idle' };
}
