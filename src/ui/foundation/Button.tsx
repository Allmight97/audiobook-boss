import { omit } from 'solid-js';
import type { JSX } from '@solidjs/web';

export type ButtonTone = 'primary' | 'secondary';

export type ButtonProps = {
	readonly tone?: ButtonTone;
} & JSX.ButtonHTMLAttributes<HTMLButtonElement>;

export function Button(props: ButtonProps): JSX.Element {
	const rest = omit(props, 'tone', 'class', 'type', 'children');
	const tone = () => props.tone ?? 'secondary';
	return (
		<button
			{...rest}
			type={props.type ?? 'button'}
			class={[
				'abb-button',
				tone() === 'primary' ? 'abb-button-primary' : 'abb-button-secondary',
				props.class,
			]}
		>
			{props.children}
		</button>
	);
}
