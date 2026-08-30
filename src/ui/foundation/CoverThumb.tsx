import type { JSX } from 'solid-js';

export type CoverThumbProps = {
	readonly class?: string;
	readonly testId?: string;
	readonly children?: JSX.Element;
};

export function CoverThumb(props: CoverThumbProps): JSX.Element {
	return (
		<div class={`abb-cover${props.class ? ` ${props.class}` : ''}`} data-testid={props.testId}>
			{props.children ?? <span>No Art</span>}
		</div>
	);
}
