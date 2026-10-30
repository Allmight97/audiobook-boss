import { describe, expect, it } from 'vitest';
import { createTypingEcho } from './typingEcho';

function deferred(): { promise: Promise<void>; resolve: () => void } {
	let resolve = () => {};
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function setup() {
	let binding = 1;
	let changes = 0;
	const echo = createTypingEcho<'title'>({
		scopeOf: () => [binding],
		changed: () => {
			changes += 1;
		},
	});
	return {
		echo,
		rebind: (next: number) => {
			binding = next;
		},
		changes: () => changes,
	};
}

describe('typing echo', () => {
	it('shows typed text before the engine replies', () => {
		const { echo, changes } = setup();
		echo.show('title', 'Al', () => deferred().promise);
		expect(echo.valueOf('title')).toBe('Al');
		expect(changes()).toBe(1);
	});

	it('drops typed text once the engine replies to it', async () => {
		const { echo } = setup();
		const reply = deferred();
		echo.show('title', 'Al', () => reply.promise);
		reply.resolve();
		await reply.promise;
		await Promise.resolve();
		expect(echo.valueOf('title')).toBeUndefined();
	});

	it('keeps newer typing when the reply to an earlier keystroke arrives', async () => {
		const { echo } = setup();
		const first = deferred();
		const second = deferred();
		echo.show('title', 'Al', () => first.promise);
		echo.show('title', 'Alp', () => second.promise);
		first.resolve();
		await first.promise;
		await Promise.resolve();
		expect(echo.valueOf('title')).toBe('Alp');
		second.resolve();
		await second.promise;
		await Promise.resolve();
		expect(echo.valueOf('title')).toBeUndefined();
	});

	it('hides typed text once the form it was typed into is replaced', () => {
		const { echo, rebind } = setup();
		echo.show('title', 'Al', () => deferred().promise);
		rebind(2);
		expect(echo.valueOf('title')).toBeUndefined();
		echo.show('title', 'Be', () => deferred().promise);
		expect(echo.valueOf('title')).toBe('Be');
	});

	it('restores nothing when a reply arrives after the echo was cleared', async () => {
		const { echo } = setup();
		const reply = deferred();
		echo.show('title', 'Al', () => reply.promise);
		echo.clear();
		expect(echo.valueOf('title')).toBeUndefined();
		reply.resolve();
		await reply.promise;
		await Promise.resolve();
		expect(echo.valueOf('title')).toBeUndefined();
	});
});
