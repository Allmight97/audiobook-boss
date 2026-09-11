import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, expect, it } from 'vitest';
import { withDevelopmentIdentity } from './tauri';

const roots: string[] = [];

function checkout(): string {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-identity-'));
	roots.push(root);
	mkdirSync(path.join(root, 'src-tauri'));
	writeFileSync(
		path.join(root, 'src-tauri/tauri.conf.json'),
		JSON.stringify({ identifier: 'com.audiobook-boss', productName: 'AudioBook Boss' }),
	);
	return root;
}

afterEach(() => {
	for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

it('isolates development storage by checkout and resolves symlinks to the same identity', () => {
	const first = checkout();
	const second = checkout();
	const alias = path.join(second, 'first-checkout');
	symlinkSync(first, alias);
	const identity = (root: string) => {
		const args = withDevelopmentIdentity(root, ['dev']);
		return JSON.parse(args[args.length - 1]);
	};
	expect(identity(first).identifier).not.toBe('com.audiobook-boss');
	expect(identity(first).identifier).not.toBe(identity(second).identifier);
	expect(identity(alias)).toEqual(identity(first));
});

it('enforces isolation after supplied config while preserving runner and app arguments', () => {
	const root = checkout();
	const args = [
		'dev',
		'--release',
		'--config',
		'{"identifier":"com.audiobook-boss"}',
		'--',
		'--locked',
		'--',
		'book.m4b',
	];
	const result = withDevelopmentIdentity(root, args);
	expect(result.slice(0, 4)).toEqual(args.slice(0, 4));
	expect(result[4]).toBe('--config');
	expect(JSON.parse(result[5]).identifier).toMatch(/^com\.audiobook-boss\.dev\./);
	expect(result.slice(6)).toEqual(args.slice(4));
	expect(withDevelopmentIdentity(root, ['build', '--debug'])).toContain('--config');
	expect(withDevelopmentIdentity(root, ['build', '-d'])).toContain('--config');
});

it('leaves production builds and other CLI commands untouched', () => {
	for (const args of [['build', '--bundles', 'app,dmg'], ['info'], ['build', '--', '--debug']]) {
		expect(withDevelopmentIdentity('/unused', args)).toEqual(args);
	}
});
