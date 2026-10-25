import type { JSX } from '@solidjs/web';

export type ProgressProps = {
	readonly value: number;
	/** Shimmer while work is underway. Idle bars stay still. */
	readonly active?: boolean;
	readonly label?: string;
	readonly fillId?: string;
	readonly class?: string;
};

export function Progress(props: ProgressProps): JSX.Element {
	const clamped = () => Math.min(100, Math.max(0, props.value));
	// A full-width bar slid inside the clipping track keeps its rounded end.
	const offset = () => `translateX(${clamped() - 100}%)`;
	return (
		<div
			class={`abb-progress${props.active ? ' is-active' : ''}${props.class ? ` ${props.class}` : ''}`}
			role="progressbar"
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={Math.round(clamped())}
			aria-label={props.label}
		>
			<div id={props.fillId} class="abb-progress-fill" style={{ transform: offset() }} />
		</div>
	);
}
