import type { JSX } from 'solid-js';

export type ProgressProps = {
	readonly value: number;
	readonly label?: string;
	readonly fillId?: string;
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
		>
			<div id={props.fillId} class="abb-progress-fill" style={{ width: `${clamped()}%` }} />
		</div>
	);
}
