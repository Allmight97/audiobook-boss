import { omit } from 'solid-js';
import type { JSX } from '@solidjs/web';

export type SelectProps = {
	readonly value: string | number;
	readonly onChoose: (value: string) => unknown;
} & Omit<JSX.SelectHTMLAttributes<HTMLSelectElement>, 'value' | 'onChange'>;

export function Select(props: SelectProps): JSX.Element {
	const rest = omit(props, 'value', 'onChoose', 'children');
	return (
		<select
			{...rest}
			value={props.value}
			onChange={async (event) => {
				const select = event.currentTarget;
				try {
					await props.onChoose(select.value);
				} finally {
					select.value = String(props.value);
				}
			}}
		>
			{props.children}
		</select>
	);
}
