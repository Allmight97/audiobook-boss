import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

export interface Finding {
	file: string;
	line: number;
	message: string;
}

interface Rule {
	appliesTo: (file: string) => boolean;
	comment?: RegExp;
	allow?: string;
	findIn: (line: string, insideCapture: boolean) => string | null;
	message: string;
}

const OWNED_SHELL = (file: string) =>
	(file.endsWith('.sh') && !file.startsWith('vendor/')) || file.startsWith('.githooks/');
const OWNED_RUST = (file: string) => file.endsWith('.rs') && !file.startsWith('vendor/');
const RUST_TEST_FILE = (file: string) =>
	/(^|\/|_)tests\.rs$/.test(file) || /(^|\/)(tests|test_cases)\//.test(file);
const ENGINE_OUTSIDE_MEDIA_WRAPPERS = (file: string) =>
	OWNED_RUST(file) &&
	file.startsWith('crates/abb-engine/src/') &&
	!file.startsWith('crates/abb-engine/src/audio/') &&
	!file.startsWith('crates/abb-engine/src/metadata/');
const CODE_ONLY = (line: string) => line.replace(/"(?:[^"\\]|\\.)*"/g, '""').replace(/\/\/.*$/, '');

const GNU_ONLY = [
	/\b(sed|grep)\b[^|]*\\[?+|]/,
	/\bsed\b[^#|]*\s-i\b/,
	/\bgrep\b[^#|]*\s-[a-zA-Z]*P/,
	/\breadlink -f\b/,
	/\bdate\b[^#|]*\s-d\b/,
	/\bstat -c\b/,
	/\s-printf\b/,
	/\bdeclare -A\b/,
	/\b(mapfile|readarray)\b/,
	/\$\{\w+(,,|\^\^)\}/,
];
const OWNED_SOURCE = (file: string) =>
	/\.(rs|ts|tsx|mjs|css|sh|toml|ya?ml)$/.test(file) &&
	!file.startsWith('vendor/') &&
	file !== 'src/lib/generated/tauri.ts';
const STAND_IN_COMMENT =
	/(?:^|\s)(?:\/\/+|#|\/\*+|\*)\s*([-=]{3,}\s*\S.*?\s*[-=]{3,}|EXCEPTION:|DO NOT REMOVE)/;

const SILENCED = /2>\s*\/dev\/null|\|\|\s*true\b/;
const CAPTURE_WITH_SILENCE = /\$\((?:[^()]|\([^()]*\))*(2>\s*\/dev\/null|\|\|\s*true\b)/;

const RULES: Rule[] = [
	{
		appliesTo: OWNED_SHELL,
		comment: /^\s*#/,
		allow: '# allow-gnu:',
		findIn: (line) => GNU_ONLY.map((pattern) => line.match(pattern)?.[0]).find(Boolean) ?? null,
		message:
			'is GNU-only or needs bash 4; macOS runs BSD tools and bash 3.2. Fix: use sed -E, awk, or a portable form, or mark the line `# allow-gnu: <why it runs only on Linux>`.',
	},
	{
		appliesTo: OWNED_SHELL,
		comment: /^\s*#/,
		allow: '# allow-silence:',
		findIn: (line, insideCapture) =>
			line.match(CAPTURE_WITH_SILENCE)?.[1] ??
			(insideCapture ? (line.match(SILENCED)?.[0] ?? null) : null),
		message:
			'hides an error inside a command substitution, so a failure reads as empty output. Fix: let it fail, capture with 2>&1, or mark the line `# allow-silence: <why the error does not matter>`.',
	},
	{
		appliesTo: OWNED_RUST,
		comment: /^\s*\/\//,
		allow: '// allow-permission-fake:',
		findIn: (line) =>
			line.match(/\b(from_mode|set_mode)\(0o[0145][0-7]{2}\)|\bchattr\b/)?.[0] ?? null,
		message:
			'fakes an I/O failure with permission bits, which root ignores (cloud containers run as root). Fix: put a file where a directory is expected, or the reverse, or mark the line `// allow-permission-fake: <why it runs unprivileged>`.',
	},
	{
		appliesTo: (file) => OWNED_RUST(file) && !RUST_TEST_FILE(file),
		comment: /^\s*\/\//,
		findIn: (line) => line.match(/^\s*(#\[test\]|#\[tokio::test|proptest!)/)?.[1] ?? null,
		message:
			'is a rust test outside a test file. Fix: move the test module to a sibling `<file>_tests.rs` (`#[cfg(test)] #[path = "<file>_tests.rs"] mod tests;`); tests live in files named `*tests.rs` and under `tests/` or `test_cases/`.',
	},
	{
		appliesTo: ENGINE_OUTSIDE_MEDIA_WRAPPERS,
		comment: /^\s*\/\//,
		findIn: (line) =>
			CODE_ONLY(line).match(/\bunsafe\s*(\{|fn\b|impl\b|trait\b|extern\b)/)?.[0] ?? null,
		message:
			'is unsafe outside the media wrappers. Fix: keep `unsafe` in the FFmpeg and FAAC wrappers under `audio/` and `metadata/` and call a safe function from here (crates/abb-engine/AGENTS.md, Runtime Constraints).',
	},
	{
		appliesTo: OWNED_SOURCE,
		findIn: (line) => line.match(STAND_IN_COMMENT)?.[1] ?? null,
		message:
			'is a comment that stands in for structure or enforcement. Fix: split the code where a banner sits, follow the rule an EXCEPTION stamp excuses, and replace a scare comment with a name, type, test, or check.',
	},
];

export function checkFile(file: string, text: string): Finding[] {
	const rules = RULES.filter((rule) => rule.appliesTo(file));
	const findings: Finding[] = [];
	let insideCapture = false;
	const lines = text.split('\n');
	lines.forEach((line, index) => {
		if (/^\s*\)/.test(line)) {
			insideCapture = false;
		}
		for (const rule of rules) {
			const { allow } = rule;
			if (
				rule.comment?.test(line) ||
				(allow && (line.includes(allow) || lines[index - 1]?.includes(allow)))
			) {
				continue;
			}
			const match = rule.findIn(line, insideCapture);
			if (match) {
				findings.push({ file, line: index + 1, message: `\`${match}\` ${rule.message}` });
			}
		}
		if (/\$\(\s*$/.test(line)) {
			insideCapture = true;
		}
	});
	return findings;
}

function main(): number {
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const tracked = execFileSync('git', ['ls-files'], { cwd: repoRoot, encoding: 'utf8' })
		.split('\n')
		.filter((file) => file && RULES.some((rule) => rule.appliesTo(file)));
	const findings = tracked.flatMap((file) =>
		checkFile(file, readFileSync(path.join(repoRoot, file), 'utf8')),
	);
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
