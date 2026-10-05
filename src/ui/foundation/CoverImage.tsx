import { createSignal } from 'solid-js';
import type { JSX } from '@solidjs/web';

export type CoverImageProps = {
	/** The cover's address; absent when the source has no cover. */
	readonly src: string | null | undefined;
	readonly alt: string;
	readonly class?: string;
	readonly testId?: string;
	/** Load at once; otherwise the cover loads as it nears the screen. */
	readonly eager?: boolean;
	/** Shown when there is no cover, or it could not be loaded. */
	readonly missing?: JSX.Element;
	/** Shown when the cover could not be loaded; defaults to `missing`. */
	readonly failed?: JSX.Element;
};

/**
 * One cover image: its container shows while it loads, it fades in once
 * loaded, and a failed load shows a state instead of a broken image.
 */
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
