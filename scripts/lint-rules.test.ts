import { execSync } from 'node:child_process';
import {
	copyFileSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const CONFIG_FILES = ['.oxlintrc.json', 'biome.json', 'tsconfig.json', 'scripts/tsconfig.json'];

interface Case {
	readonly name: string;
	readonly file: string;
	readonly source: string;
	readonly rule: string | null;
	readonly message?: string;
}

function branches(count: number): string {
	const ifs = Array.from(
		{ length: count },
		(_, index) => `\tif (value === ${index}) return ${index};`,
	);
	return `export function pick(value: number): number {\n${ifs.join('\n')}\n\treturn -1;\n}\n`;
}

function importing(specifier: string): string {
	return `import { probe } from '${specifier}';\n\nexport const value = probe;\n`;
}

function restricted(file: string, specifier: string, message: string): Case {
	return {
		name: `${file} rejects ${specifier}`,
		file,
		source: importing(specifier),
		rule: 'eslint(no-restricted-imports)',
		message,
	};
}

function allowed(file: string, specifier: string): Case {
	return { name: `${file} allows ${specifier}`, file, source: importing(specifier), rule: null };
}

const floating =
	'async function work(): Promise<void> {}\n\nexport function run(): void {\n\twork();\n}\n';
const awaited =
	'async function work(): Promise<void> {}\n\nexport function run(): void {\n\tvoid work();\n}\n';
const nonNull =
	'export function first(values: string[]): string {\n\treturn values.find(Boolean)!;\n}\n';
const kinds = "type Kind = 'a' | 'b';\n\n";

const OXLINT_CASES: Case[] = [
	{
		name: 'complexity 21 fails',
		file: 'src/lib/probe/complex.ts',
		source: branches(20),
		rule: 'eslint(complexity)',
	},
	{
		name: 'complexity 20 passes',
		file: 'src/lib/probe/simple.ts',
		source: branches(19),
		rule: null,
	},
	restricted('src/lib/probe/ts.ts', 'typescript', 'scripts/AGENTS.md'),
	restricted('src/lib/probe/tailwind.ts', 'tailwindcss', 'src/ui/foundation/AGENTS.md'),
	restricted(
		'src/lib/probe/internal.ts',
		'../../ui/foundation/internal',
		'src/ui/foundation/AGENTS.md',
	),
	restricted('src/lib/probe/owner.ts', '../../app/encoding/project', 'src/app/AGENTS.md'),
	restricted('src/lib/probe/commands.ts', '../tauri/commands', 'src/lib/tauri/AGENTS.md'),
	restricted('src/lib/probe/plugin.ts', '@tauri-apps/plugin-dialog', 'src/lib/tauri/AGENTS.md'),
	allowed('src/lib/probe/ownerRoot.ts', '../../app/encoding'),
	allowed('src/lib/probe/foundation.ts', '../../ui/foundation'),
	allowed('src/mainProbe.tsx', './app/runtime/ProductionRoot'),
	restricted('src/mainProbeOther.tsx', './app/runtime/state', 'src/app/AGENTS.md'),
	restricted(
		'src/app/probe/internal.ts',
		'../../ui/foundation/internal',
		'src/ui/foundation/AGENTS.md',
	),
	restricted('src/ui/ProbeTop.tsx', './fileList/FileListView', 'src/ui/AGENTS.md'),
	allowed('src/ui/ProbeTopRoot.tsx', './fileList'),
	restricted('src/ui/probeView/Nested.tsx', '../fileList/FileListView', 'src/ui/AGENTS.md'),
	allowed('src/ui/probeView/NestedRoot.tsx', '../fileList'),
	allowed('src/ui/foundation/probeSibling.ts', '../fileList/FileListView'),
	restricted('src/ui/foundation/probeEvent.ts', '@tauri-apps/api/event', 'src/lib/tauri/AGENTS.md'),
	allowed('src/lib/tauri/probePlugin.ts', '@tauri-apps/plugin-dialog'),
	restricted('src/lib/tauri/probeTailwind.ts', 'tailwindcss', 'src/ui/foundation/AGENTS.md'),
	{
		name: 'an import cycle fails',
		file: 'src/lib/probe/cycleA.ts',
		source: "import { b } from './cycleB';\n\nexport const a = (): number => b() + 1;\n",
		rule: 'import(no-cycle)',
	},
	{
		name: 'the other side of the cycle',
		file: 'src/lib/probe/cycleB.ts',
		source: "import { a } from './cycleA';\n\nexport const b = (): number => a() - 1;\n",
		rule: 'import(no-cycle)',
	},
	{
		name: 'a one-way import passes',
		file: 'src/lib/probe/chainA.ts',
		source: "import { b } from './chainB';\n\nexport const a = (): number => b() + 1;\n",
		rule: null,
	},
	{
		name: 'the end of the chain',
		file: 'src/lib/probe/chainB.ts',
		source: 'export const b = (): number => 1;\n',
		rule: null,
	},
	{
		name: 'node modules fail in src',
		file: 'src/lib/probe/node.ts',
		source: "import { sep } from 'node:path';\n\nexport const separator = sep;\n",
		rule: 'import(no-nodejs-modules)',
	},
	{
		name: 'node modules pass in scripts',
		file: 'scripts/probe-node.ts',
		source: "import { sep } from 'node:path';\n\nexport const separator = sep;\n",
		rule: null,
	},
	{
		name: 'explicit any fails',
		file: 'src/lib/probe/any.ts',
		source: 'export const value: any = 1;\n',
		rule: 'typescript(no-explicit-any)',
	},
	{
		name: 'unknown passes',
		file: 'src/lib/probe/unknown.ts',
		source: 'export const value: unknown = 1;\n',
		rule: null,
	},
	{
		name: 'a floating promise fails in src',
		file: 'src/lib/probe/floating.ts',
		source: floating,
		rule: 'typescript(no-floating-promises)',
	},
	{
		name: 'a voided promise passes in src',
		file: 'src/lib/probe/voided.ts',
		source: awaited,
		rule: null,
	},
	{
		name: 'a floating promise fails in scripts',
		file: 'scripts/probe-floating.ts',
		source: floating,
		rule: 'typescript(no-floating-promises)',
	},
	{
		name: 'a voided promise passes in scripts',
		file: 'scripts/probe-voided.ts',
		source: awaited,
		rule: null,
	},
	{
		name: 'a promise as a condition fails',
		file: 'src/lib/probe/misused.ts',
		source:
			'const ready = Promise.resolve(true);\n\nexport function check(): boolean {\n\treturn ready ? true : false;\n}\n',
		rule: 'typescript(no-misused-promises)',
	},
	{
		name: 'an awaited condition passes',
		file: 'src/lib/probe/awaitedCondition.ts',
		source:
			'const ready = Promise.resolve(true);\n\nexport async function check(): Promise<boolean> {\n\treturn await ready;\n}\n',
		rule: null,
	},
	{
		name: 'a missing case fails',
		file: 'src/lib/probe/switchMissing.ts',
		source: `${kinds}export function label(kind: Kind): string {\n\tswitch (kind) {\n\t\tcase 'a':\n\t\t\treturn 'A';\n\t}\n\treturn '';\n}\n`,
		rule: 'typescript(switch-exhaustiveness-check)',
	},
	{
		name: 'every case passes',
		file: 'src/lib/probe/switchFull.ts',
		source: `${kinds}export function label(kind: Kind): string {\n\tswitch (kind) {\n\t\tcase 'a':\n\t\t\treturn 'A';\n\t\tcase 'b':\n\t\t\treturn 'B';\n\t}\n}\n`,
		rule: null,
	},
	{
		name: 'a default case passes',
		file: 'src/lib/probe/switchDefault.ts',
		source: `${kinds}export function label(kind: Kind): string {\n\tswitch (kind) {\n\t\tcase 'a':\n\t\t\treturn 'A';\n\t\tdefault:\n\t\t\treturn '';\n\t}\n}\n`,
		rule: null,
	},
	{
		name: 'a non-null assertion fails in src',
		file: 'src/lib/probe/nonNull.ts',
		source: nonNull,
		rule: 'typescript(no-non-null-assertion)',
	},
	{
		name: 'a non-null assertion passes in a test',
		file: 'src/lib/probe/nonNull.test.ts',
		source: nonNull,
		rule: null,
	},
];

const BIOME_CASES: Case[] = [
	{
		name: 'an unknown CSS property fails',
		file: 'src/probe/unknown.css',
		source: 'a {\n\tcolr: red;\n}\n',
		rule: 'lint/correctness/noUnknownProperty',
	},
	{
		name: 'a known CSS property passes',
		file: 'src/probe/known.css',
		source: 'a {\n\tcolor: red;\n}\n',
		rule: null,
	},
	{
		name: 'a duplicate CSS property fails',
		file: 'src/probe/duplicate.css',
		source: 'a {\n\tcolor: red;\n\tcolor: blue;\n}\n',
		rule: 'lint/suspicious/noDuplicateProperties',
	},
	{
		name: 'distinct CSS properties pass',
		file: 'src/probe/distinct.css',
		source: 'a {\n\tcolor: red;\n\tmargin: 0;\n}\n',
		rule: null,
	},
	{
		name: 'an invalid grid area fails',
		file: 'src/probe/grid.css',
		source: '.g {\n\tgrid-template-areas:\n\t\t"a a"\n\t\t"b";\n}\n',
		rule: 'lint/correctness/noInvalidGridAreas',
	},
	{
		name: 'a valid grid area passes',
		file: 'src/probe/gridValid.css',
		source: '.g {\n\tgrid-template-areas:\n\t\t"a a"\n\t\t"b b";\n}\n',
		rule: null,
	},
	{
		name: 'a duplicate JSON key fails',
		file: 'src/probe/duplicate.json',
		source: '{ "a": 1, "a": 2 }\n',
		rule: 'lint/suspicious/noDuplicateObjectKeys',
	},
	{
		name: 'distinct JSON keys pass',
		file: 'src/probe/distinct.json',
		source: '{ "a": 1, "b": 2 }\n',
		rule: null,
	},
];

interface Finding {
	readonly file: string;
	readonly rule: string;
	readonly message: string;
}

function productionCommands(): string[] {
	const pkg = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')) as {
		scripts: Record<string, string>;
	};
	return pkg.scripts['lint:check'].split(' && ');
}

function run(root: string, command: string): string {
	try {
		return execSync(command, {
			cwd: root,
			encoding: 'utf8',
			env: { ...process.env, PATH: `${path.join(root, 'node_modules/.bin')}:${process.env.PATH}` },
			stdio: ['ignore', 'pipe', 'pipe'],
		});
	} catch (error) {
		return (error as { stdout: string }).stdout;
	}
}

function oxlintFindings(root: string, command: string): Finding[] {
	const report = JSON.parse(run(root, `${command} -f json`)) as {
		diagnostics: { filename: string; code: string; message: string; help?: string }[];
	};
	return report.diagnostics.map((d) => ({
		file: d.filename,
		rule: d.code,
		message: `${d.message} ${d.help ?? ''}`,
	}));
}

function biomeFindings(root: string, command: string): Finding[] {
	const report = JSON.parse(run(root, `${command} --reporter=json`)) as {
		diagnostics: { category: string; location: { path: string } }[];
	};
	return report.diagnostics.map((d) => ({ file: d.location.path, rule: d.category, message: '' }));
}

let root = '';
let findings: Finding[] = [];

beforeAll(() => {
	root = mkdtempSync(path.join(os.tmpdir(), 'abb-lint-rules-'));
	for (const file of CONFIG_FILES) {
		mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
		copyFileSync(path.join(repoRoot, file), path.join(root, file));
	}
	symlinkSync(path.join(repoRoot, 'node_modules'), path.join(root, 'node_modules'));
	for (const entry of [...OXLINT_CASES, ...BIOME_CASES]) {
		mkdirSync(path.dirname(path.join(root, entry.file)), { recursive: true });
		writeFileSync(path.join(root, entry.file), entry.source);
	}
	writeFileSync(path.join(root, '.gitignore'), 'node_modules\n');
	execSync('git init -q', { cwd: root });
	const commands = productionCommands();
	const oxlint = commands.find((command) => command.startsWith('oxlint '));
	const biome = commands.find((command) => command.startsWith('biome '));
	if (!oxlint || !biome) throw new Error('lint:check no longer runs oxlint and biome');
	findings = [...oxlintFindings(root, oxlint), ...biomeFindings(root, biome)];
}, 120_000);

afterAll(() => {
	rmSync(root, { recursive: true, force: true });
});

function findingsFor(file: string): Finding[] {
	return findings.filter((finding) => finding.file === file);
}

describe.each([
	['Oxlint', OXLINT_CASES],
	['Biome', BIOME_CASES],
])('%s rules through lint:check', (_tool, cases) => {
	it.each(cases.filter((entry) => entry.rule !== null))('$name', (entry) => {
		const found = findingsFor(entry.file).find((finding) => finding.rule === entry.rule);
		expect(found, `${entry.file} should report ${entry.rule}`).toBeDefined();
		expect(found?.message).toContain(entry.message ?? '');
	});

	it.each(cases.filter((entry) => entry.rule === null))('$name', (entry) => {
		expect(findingsFor(entry.file)).toEqual([]);
	});
});
