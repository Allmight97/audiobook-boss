import type {
	ChoiceAvailability,
	CollisionReview,
	PlannedOutput,
	RenameRefusal,
} from '../../lib/generated/tauri';

export type CollisionView = {
	readonly reviewId: number | null;
	readonly outputs: ReadonlyArray<PlannedOutput>;
	readonly title: string;
	readonly body: string;
	readonly replaceRefusal: string | null;
	readonly renameRefusal: string | null;
};

const RENAME_REFUSALS: Record<RenameRefusal, string> = {
	tooLong: 'a numbered name would be too long for a file name',
	exhausted: 'every numbered name from -1 to -99 is already taken',
};

function replaceRefusal(choice: ChoiceAvailability): string | null {
	if (choice.kind === 'available') return null;
	return `Overwrite Existing is unavailable: titles in this export share a name, so one must be renamed, and ${RENAME_REFUSALS[choice.reason]}.`;
}

function renameRefusal(choice: ChoiceAvailability): string | null {
	if (choice.kind === 'available') return null;
	return `Keep Existing is unavailable: ${RENAME_REFUSALS[choice.reason]}.`;
}

export function collisionView(review: CollisionReview | null): CollisionView {
	const outputs = review?.outputs ?? [];
	const count = outputs.length;
	return {
		reviewId: review?.reviewId ?? null,
		outputs,
		title: 'Resolve Existing File Conflicts',
		body: !review
			? ''
			: count === 1
				? '1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?'
				: `${count} files with the same name already exist in the target output folders. How do you want to resolve the conflicts?`,
		replaceRefusal: review ? replaceRefusal(review.choices.replace) : null,
		renameRefusal: review ? renameRefusal(review.choices.rename) : null,
	};
}
