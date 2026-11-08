import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const checkScriptPath = path.join(scriptDir, 'check-tauri-runtime-boundary.ts');
const TEST_PATH = process.env.PATH ?? '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin';

type CheckResult = {
	error?: string;
	signal: NodeJS.Signals | null;
	status: number | null;
	stdout: string;
	stderr: string;
};

function createFixtureRepo(): string {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-tauri-runtime-boundary-'));

	for (const dir of [
		'src/lib/generated',
		'src/lib/tauri',
		'src/types',
		'src/test',
		'src/ui/__tests__',
	]) {
		mkdirSync(path.join(repoRoot, dir), { recursive: true });
	}

	writeFileSync(
		path.join(repoRoot, 'src/lib/generated/tauri.ts'),
		'export const commands = {}; export const events = {}; export type Foo = string;\n',
	);
	writeFileSync(
		path.join(repoRoot, 'src/lib/tauri/commands.ts'),
		"import { commands as generatedCommands } from '../generated/tauri';\nvoid generatedCommands;\n",
	);
	writeFileSync(
		path.join(repoRoot, 'src/lib/tauri/client.ts'),
		"import { events as generatedEvents } from '../generated/tauri';\nvoid generatedEvents;\n",
	);
	writeFileSync(
		path.join(repoRoot, 'src/types/session.ts'),
		"import type { Foo } from '../lib/generated/tauri';\nexport type Bar = Foo;\n",
	);

	return repoRoot;
}

function runTauriBoundaryCheck(repoRoot: string): CheckResult {
	const result = spawnSync('bun', [checkScriptPath], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			...process.env,
			PATH: TEST_PATH,
		},
	});
	return {
		error: result.error?.message,
		signal: result.signal,
		status: result.status,
		stdout: result.stdout,
		stderr: result.stderr,
	};
}

function expectStatus(result: CheckResult, status: number): void {
	if (result.status !== status) {
		throw new Error(
			[
				`Expected status ${status}, received ${String(result.status)}`,
				`signal: ${String(result.signal)}`,
				`error: ${result.error ?? ''}`,
				'stdout:',
				result.stdout,
				'stderr:',
				result.stderr,
			].join('\n'),
		);
	}
}

describe('check-tauri-runtime-boundary.ts', () => {
	it('allows boundary-owned generated value imports and type-only contract imports', () => {
		const repoRoot = createFixtureRepo();
		try {
			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[check-tauri-runtime-boundary] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('ignores commented generated imports', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/commented.ts'),
				[
					"// import { commands } from '../lib/generated/tauri';",
					'/*',
					"import { events } from '../lib/generated/tauri';",
					'*/',
					'export const ok = true;',
					'',
				].join('\n'),
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[check-tauri-runtime-boundary] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects generated star re-exports', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/bypass.ts'),
				"export * from '../lib/generated/tauri';\n",
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('generated Tauri value re-exports');
			expect(result.stderr).toContain('src/ui/bypass.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects generated namespace value imports after default imports', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/bypass.ts'),
				"import defaultExport, * as generated from '../lib/generated/tauri';\nvoid defaultExport;\nvoid generated;\n",
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('generated Tauri namespace value imports');
			expect(result.stderr).toContain('src/ui/bypass.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects a raw Tauri core namespace import in runtime UI code', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/rawCore.ts'),
				"import * as core from '@tauri-apps/api/core';\nvoid core;\n",
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('raw Tauri core namespace imports');
			expect(result.stderr).toContain('src/ui/rawCore.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('allows a raw Tauri core namespace import in the boundary and in tests', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/lib/tauri/rawCore.ts'),
				"import * as core from '@tauri-apps/api/core';\nvoid core;\n",
			);
			writeFileSync(
				path.join(repoRoot, 'src/ui/__tests__/rawCore.test.ts'),
				"import * as core from '@tauri-apps/api/core';\nvoid core;\n",
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[check-tauri-runtime-boundary] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects raw __TAURI_INVOKE usage in runtime app code', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/globalInvoke.ts'),
				'window.__TAURI_INVOKE("process_audiobook_files");\n',
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('raw __TAURI_INVOKE usage');
			expect(result.stderr).toContain('src/ui/globalInvoke.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects raw __TAURI_INVOKE usage inside template interpolations', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/templateInvoke.ts'),
				'`${window.__TAURI_INVOKE("process_audiobook_files")}`;\n',
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('raw __TAURI_INVOKE usage');
			expect(result.stderr).toContain('src/ui/templateInvoke.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('ignores generated-import text inside string literals', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/helpText.ts'),
				'const help = "import { commands } from \'../lib/generated/tauri\'";\nexport const ok = help;\n',
			);

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[check-tauri-runtime-boundary] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('allows a type-only generated re-export after a side-effect import', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/types.ts'),
				"import './setup';\nexport type { commands } from '../lib/generated/tauri';\n",
			);
			writeFileSync(path.join(repoRoot, 'src/ui/setup.ts'), 'export {};\n');

			const result = runTauriBoundaryCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[check-tauri-runtime-boundary] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
