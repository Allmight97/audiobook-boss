import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(import.meta.dirname, '..');
const read = (file: string) => readFileSync(path.join(repoRoot, file), 'utf8');

const kebab = (name: string) => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

export function requiredPermissions(source: string): string[] {
	const imports = source.matchAll(
		/import\s*\{([^}]*)\}\s*from\s*'@tauri-apps\/(?:plugin-([a-z-]+)|api\/(event))'/g,
	);
	return [...imports].flatMap(([, names = '', plugin, api]) =>
		names
			.split(',')
			.map((name) => name.trim())
			.filter((name) => name && !name.startsWith('type '))
			.map((name) => name.split(/\s+as\s+/)[0] ?? name)
			.map((name) =>
				plugin ? `${plugin}:allow-${kebab(name)}` : `core:${api}:allow-${kebab(name)}`,
			),
	);
}

type Capability = { permissions: (string | { identifier: string })[] };

function readCapability(): Capability {
	return JSON.parse(read('src-tauri/capabilities/default.json')) as Capability;
}

function grantedPermissions(): Set<string> {
	return new Set(
		readCapability().permissions.map((entry) =>
			typeof entry === 'string' ? entry : entry.identifier,
		),
	);
}

function minor(version: string): string {
	return version
		.replace(/^[^\d]*/, '')
		.split('.')
		.slice(0, 2)
		.join('.');
}

describe('Tauri contract between the frontend and src-tauri', () => {
	it('derives one permission per imported plugin or event function', () => {
		expect(
			requiredPermissions(
				"import { open, save as saveFile } from '@tauri-apps/plugin-dialog';\nimport { type Event, listen } from '@tauri-apps/api/event';",
			),
		).toEqual(['dialog:allow-open', 'dialog:allow-save', 'core:event:allow-listen']);
	});

	it('grants every plugin and event function the frontend imports', () => {
		const sources = execFileSync('git', ['ls-files', 'src/**/*.ts', 'src/**/*.tsx'], {
			cwd: repoRoot,
			encoding: 'utf8',
		})
			.split('\n')
			.filter((file) => file && !/\.test\.tsx?$/.test(file));
		const granted = grantedPermissions();
		const missing = sources.flatMap((file) =>
			requiredPermissions(read(file))
				.filter((permission) => !granted.has(permission))
				.map((permission) => `${file}: ${permission}`),
		);
		expect(missing, 'Add each permission to src-tauri/capabilities/default.json').toEqual([]);
	});

	it('lets the window open http and https URLs and no file path', () => {
		const { permissions } = readCapability();
		expect(
			permissions.find(
				(permission) =>
					typeof permission === 'object' && permission.identifier === 'opener:allow-open-url',
			),
		).toEqual({
			identifier: 'opener:allow-open-url',
			allow: [{ url: 'http://*' }, { url: 'https://*' }],
		});
		expect(
			permissions.some(
				(permission) =>
					(typeof permission === 'object' ? permission.identifier : permission) ===
					'opener:allow-open-path',
			),
		).toBe(false);
	});

	it('pins @tauri-apps/api to the minor of the Rust tauri crate', () => {
		const pkg = JSON.parse(read('package.json')) as { overrides: Record<string, string> };
		const lock =
			/\[\[package\]\]\nname = "tauri"\nversion = "([^"]+)"/.exec(read('Cargo.lock'))?.[1] ?? '';
		expect(minor(pkg.overrides['@tauri-apps/api'] ?? '')).toBe(minor(lock));
	});
});
