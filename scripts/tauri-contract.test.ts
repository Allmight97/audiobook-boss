import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(import.meta.dirname, '..');
const read = (file: string) => readFileSync(path.join(repoRoot, file), 'utf8');

const kebab = (name: string) => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

function requiredPermissions(source: string): string[] {
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

const pascal = (name: string) => name.charAt(0).toUpperCase() + name.slice(1);

function trackedFiles(...patterns: string[]): string[] {
	return execFileSync('git', ['ls-files', ...patterns], { cwd: repoRoot, encoding: 'utf8' })
		.split('\n')
		.filter(Boolean);
}

function intentKinds(generated: string, union: string): string[] {
	const start = generated.indexOf(`export type ${union} =`);
	const end = generated.indexOf('\nexport type ', start + 1);
	return [...generated.slice(start, end).matchAll(/\bkind: "(\w+)"/g)].flatMap(([, kind]) =>
		kind ? [kind] : [],
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

type TauriConfig = { bundle: { fileAssociations: { ext: string[] }[] } };

function fileAssociationExtensions(config: TauriConfig): string[] {
	return config.bundle.fileAssociations.flatMap((association) => association.ext).toSorted();
}

function supportedImportExtensions(generated: string): string[] {
	const list = /export const SUPPORTED_AUDIO_IMPORT_EXTENSIONS = (\[[^\]]*\])/.exec(generated)?.[1];
	return (JSON.parse(list ?? '[]') as string[]).toSorted();
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

	it('has a production sender for every wire intent', () => {
		const generated = read('src/lib/generated/tauri.ts');
		const frontend = trackedFiles('src/**/*.ts', 'src/**/*.tsx')
			.filter((file) => !/\.test\.tsx?$|^src\/lib\/generated\/|^src\/test\/|__tests__/.test(file))
			.map(read)
			.join('\n');
		const host = trackedFiles('src-tauri/src/*.rs', 'src-tauri/src/**/*.rs')
			.filter((file) => !file.endsWith('_tests.rs'))
			.map(read)
			.join('\n');
		const unsent = ['SessionIntent', 'SettingsIntent', 'RemoteUiIntent'].flatMap((union) => {
			const kinds = intentKinds(generated, union);
			expect(kinds.length, `${union} yields no intent kinds; was it renamed?`).toBeGreaterThan(0);
			return kinds
				.filter(
					(kind) =>
						!new RegExp(`\\bkind:\\s*['"]${kind}['"]`).test(frontend) &&
						!new RegExp(`\\b${union}::${pascal(kind)}\\b`).test(host),
				)
				.map((kind) => `${union}.${kind}`);
		});
		expect(unsent, 'Remove each intent from the Rust enum, or send it from non-test code').toEqual(
			[],
		);
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

	it('offers Open with ABB for exactly the audio extensions the engine imports', () => {
		const config = JSON.parse(read('src-tauri/tauri.conf.json')) as TauriConfig;
		const supported = supportedImportExtensions(read('src/lib/generated/tauri.ts'));
		expect(
			supported.length,
			'SUPPORTED_AUDIO_IMPORT_EXTENSIONS was renamed or emptied',
		).toBeGreaterThan(0);
		expect(
			fileAssociationExtensions(config),
			'Make bundle.fileAssociations in src-tauri/tauri.conf.json list the same extensions as SUPPORTED_AUDIO_IMPORT_EXTENSIONS',
		).toEqual(supported);
	});
});
