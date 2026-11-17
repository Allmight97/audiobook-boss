import { createSignal } from 'solid-js';
import type { JSX } from '@solidjs/web';

export type CoverImageProps = {
	readonly src: string | null | undefined;
	readonly alt: string;
	readonly class?: string;
	readonly testId?: string;
	readonly eager?: boolean;
	readonly missing?: JSX.Element;
	readonly failed?: JSX.Element;
};

export function CoverImage(props: CoverImageProps): JSX.Element {
	const [settled, setSettled] = createSignal<{ src: string; ok: boolean } | null>(null);
	const state = () => {
		const outcome = settled();
		if (!props.src) return 'missing';
		if (outcome?.src !== props.src) return 'loading';
		return outcome.ok ? 'ready' : 'failed';
	};
	const settle = (ok: boolean) => {
		if (props.src) setSettled({ src: props.src, ok });
	};
	return (
		<>
			{state() === 'missing'
				? props.missing
				: state() === 'failed'
					? (props.failed ?? props.missing)
					: null}
			{props.src && state() !== 'failed' ? (
				<img
					src={props.src}
					alt={props.alt}
					class={`abb-cover-image${props.class ? ` ${props.class}` : ''}`}
					data-state={state()}
					data-testid={props.testId}
					loading={props.eager ? 'eager' : 'lazy'}
					decoding="async"
					onLoad={() => settle(true)}
					onError={() => settle(false)}
				/>
			) : null}
		</>
	);
}
