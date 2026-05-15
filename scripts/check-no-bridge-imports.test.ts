import {
	chmodSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'bun:test';

const SYSTEM_BASH = '/bin/bash';
const TEST_PATH = process.env.PATH ?? '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin';

type CheckResult = {
	error?: string;
	signal: NodeJS.Signals | null;
	status: number | null;
	stdout: string;
	stderr: string;
};

function createFixtureRepo(): string {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-no-bridge-'));

	for (const dir of [
		'scripts',
		'src/lib/generated',
		'src/lib/tauri',
		'src/types',
		'src/ui/outputPanel',
		'src/ui/__tests__',
	]) {
		mkdirSync(path.join(repoRoot, dir), { recursive: true });
	}

	for (const scriptName of ['check-no-bridge-imports.sh', 'check-generated-tauri-imports.ts']) {
		const sourcePath = path.join(import.meta.dir, scriptName);
		const targetPath = path.join(repoRoot, 'scripts', scriptName);
		writeFileSync(targetPath, readFileSync(sourcePath, 'utf8'));
		if (scriptName.endsWith('.sh')) {
			chmodSync(targetPath, 0o755);
		}
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
		path.join(repoRoot, 'src/types/ipc.ts'),
		"import type { Foo } from '../lib/generated/tauri';\nexport type Bar = Foo;\n",
	);

	return repoRoot;
}

function runNoBridgeCheck(repoRoot: string): CheckResult {
	const stdoutPath = path.join(repoRoot, '.no-bridge.stdout');
	const stderrPath = path.join(repoRoot, '.no-bridge.stderr');

	for (const outputPath of [stdoutPath, stderrPath]) {
		try {
			unlinkSync(outputPath);
		} catch {}
	}

	const result = spawnSync(
		SYSTEM_BASH,
		['-c', './scripts/check-no-bridge-imports.sh > .no-bridge.stdout 2> .no-bridge.stderr'],
		{
			cwd: repoRoot,
			encoding: 'utf8',
			env: {
				...process.env,
				PATH: TEST_PATH,
			},
		},
	);
	return {
		error: result.error?.message,
		signal: result.signal,
		status: result.status,
		stdout: readFileSync(stdoutPath, 'utf8'),
		stderr: readFileSync(stderrPath, 'utf8'),
	};
}

function expectStatus(result: CheckResult, status: number): void {
	if (result.status !== status) {
		throw new Error(
			[
				`Expected status ${status}, received ${String(result.status)}`,
				`signal: ${String(result.signal)}`,
				`error: ${String(result.error ?? '')}`,
				'stdout:',
				result.stdout,
				'stderr:',
				result.stderr,
			].join('\n'),
		);
	}
}

describe('check-no-bridge-imports.sh', () => {
	it('allows boundary-owned generated value imports and type-only contract imports', () => {
		const repoRoot = createFixtureRepo();
		try {
			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[no-bridge] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects generated commands value imports regardless of alias', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/bypass.ts'),
				"import { commands as c } from '../lib/generated/tauri';\nvoid c;\n",
			);

			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain("generated 'commands' value imports");
			expect(result.stderr).toContain('src/ui/bypass.ts');
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

			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 0);
			expect(result.stdout).toContain('[no-bridge] OK');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects generated value re-exports', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/bypass.ts'),
				"export { commands as generatedCommands } from '../lib/generated/tauri';\n",
			);

			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain("generated 'commands' value imports");
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

			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('generated Tauri namespace value imports');
			expect(result.stderr).toContain('src/ui/bypass.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects frontend output path naming mirror reintroduction', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/outputPanel/pathBuilder.ts'),
				'export function calculateOutputPath(): string { return ""; }\n',
			);

			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain('Frontend output path naming mirrors must not exist');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('rejects frontend output path naming mirror shorthand methods', () => {
		const repoRoot = createFixtureRepo();
		try {
			writeFileSync(
				path.join(repoRoot, 'src/ui/outputPanel/utils.ts'),
				'export const utils = { calculateOutputPath(metadata: unknown): string { void metadata; return ""; } };\n',
			);

			const result = runNoBridgeCheck(repoRoot);
			expectStatus(result, 1);
			expect(result.stderr).toContain(
				'Output path naming must stay in the Rust output_artifact boundary',
			);
			expect(result.stderr).toContain('src/ui/outputPanel/utils.ts');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
