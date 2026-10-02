/** Status-panel wording of engine progress; run truth remains in engine snapshots. */
export type DisplayStage =
	| 'analyzing'
	| 'converting'
	| 'writing'
	| 'completed'
	| 'skipped'
	| 'failed'
	| 'cancelled';
export type ActiveEventStage = Extract<DisplayStage, 'analyzing' | 'converting' | 'writing'>;

export type ProcessingStatus =
	| { stage: 'idle'; percentage: 0; message: string }
	| {
			stage: ActiveEventStage;
			percentage: number;
			message: string;
			currentFile?: string;
			etaSeconds?: number;
	  }
	| { stage: 'completed'; percentage: number; message: string }
	| { stage: 'skipped'; percentage: number; message: string }
	| { stage: 'failed'; percentage: number; message: string }
	| { stage: 'cancelled'; percentage: number; message: string };

export type JobStatus = 'queued' | 'processing' | 'completed' | 'skipped' | 'failed' | 'cancelled';

export function createInitialStatus(): ProcessingStatus {
	return {
		stage: 'idle',
		percentage: 0,
		message: 'Ready to process audiobook',
	};
}

export function isActiveEventStage(stage: ProcessingStatus['stage']): stage is ActiveEventStage {
	return stage === 'analyzing' || stage === 'converting' || stage === 'writing';
}

/**
 * Factory that constructs the correct `ProcessingStatus` variant based on the
 * `stage` discriminant. `active` extras are applied only when `stage` is an
 * active event stage; for terminal and idle stages the extras are ignored at
 * the type level. Idle is always normalized to `percentage: 0`, even if an
 * out-of-band caller passes a stale numeric value.
 */
export function buildStatus(
	stage: ProcessingStatus['stage'],
	percentage: number,
	message: string,
	active?: { currentFile?: string | null; etaSeconds?: number | null },
): ProcessingStatus {
	if (stage === 'idle') {
		return { stage, percentage: 0, message };
	}

	if (isActiveEventStage(stage)) {
		const currentFile = active?.currentFile ?? undefined;
		const etaSeconds = active?.etaSeconds ?? undefined;
		return {
			stage,
			percentage,
			message,
			...(currentFile !== undefined ? { currentFile } : {}),
			...(etaSeconds !== undefined ? { etaSeconds } : {}),
		};
	}
	return { stage, percentage, message };
}
