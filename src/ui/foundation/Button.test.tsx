import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { Button } from './Button';

describe('Button', () => {
	afterEach(() => cleanup());

	it('exposes native disabled and busy state', () => {
		render(() => (
			<Button tone="primary" busy>
				Save
			</Button>
		));
		const button = screen.getByRole('button', { name: 'Save' });
		expect(button).toBeDisabled();
		expect(button).toHaveAttribute('aria-busy', 'true');
	});
});
