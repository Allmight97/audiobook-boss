import type { JSX } from '@solidjs/web';

export type ProgressProps = {
	readonly value: number;
	readonly label: string;
	/** Work is running now; only active progress shows motion. */
	readonly active?: boolean;
	readonly class?: string;
};

export function Progress(props: ProgressProps): JSX.Element {
	const clamped = () => Math.min(100, Math.max(0, props.value));
	return (
		<div
			class={`abb-progress${props.class ? ` ${props.class}` : ''}`}
			role="progressbar"
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={Math.round(clamped())}
			aria-label={props.label}
			data-active={props.active ? '' : undefined}
		>
			<div class="abb-progress-fill" style={{ transform: `translateX(${clamped() - 100}%)` }} />
		</div>
	);
}
