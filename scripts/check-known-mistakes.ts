/**
 * Fails on code shapes behind repeated past failures in this repo. Each rule
 * names the fix and an allow marker, on the line or the line above, for a
 * line where the shape is intended; the marker gives the reason.
 *
 * Usage: bun scripts/check-known-mistakes.ts
 */
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
	comment: RegExp;
	allow: string;
	findIn: (line: string, insideCapture: boolean) => string | null;
	message: string;
}

const OWNED_SHELL = (file: string) => file.endsWith('.sh') && !file.startsWith('vendor/');
// All owned Rust, because test modules also live inline in source files.
const OWNED_RUST = (file: string) => file.endsWith('.rs') && !file.startsWith('vendor/');

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
			if (
				rule.comment.test(line) ||
				line.includes(rule.allow) ||
				lines[index - 1]?.includes(rule.allow)
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
