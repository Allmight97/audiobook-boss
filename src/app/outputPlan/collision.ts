import type { CollisionReview } from '../../types/session';
import type { PlannedOutput } from '../../types/audio';

export type CollisionView = {
	readonly isOpen: boolean;
	readonly reviewId: number | null;
	readonly outputs: ReadonlyArray<PlannedOutput>;
	readonly title: string;
	readonly body: string;
};

/** Presentation only: a held engine question has no frontend continuation. */
export function collisionView(review: CollisionReview | null): CollisionView {
	const outputs = review?.outputs ?? [];
	const count = outputs.length;
	return {
		isOpen: review !== null,
		reviewId: review?.reviewId ?? null,
		outputs,
		title: 'Resolve Existing File Conflicts',
		body: !review
			? ''
			: count === 1
				? '1 file with the same name already exists in the target output folder. How do you want to resolve the conflict?'
				: `${count} files with the same name already exist in the target output folders. How do you want to resolve the conflicts?`,
	};
}
