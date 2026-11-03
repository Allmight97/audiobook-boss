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
const SOLID_SOURCE = (file: string) => file.startsWith('src/') && file.endsWith('.tsx');
const DESTRUCTURED_PROPS =
	/\bfunction [A-Z]\w*\s*\(\s*\{|\b(?:const|let) [A-Z]\w*\s*=\s*\(\s*\{|\b(?:const|let)\s*\{[^}]*\}\s*=\s*props\b/;
const ENGINE_DIR = (dir: string) => (file: string) =>
	OWNED_RUST(file) && !RUST_TEST_FILE(file) && file.startsWith(`crates/abb-engine/src/${dir}/`);
const PURE_CORE = (file: string) =>
	OWNED_RUST(file) && !RUST_TEST_FILE(file) && /^crates\/abb-[a-z-]+-core\/src\//.test(file);
const PROVIDER_NEUTRAL_OWNERS = ['processing', 'audio', 'metadata', 'output_artifact'];
const SCRIPT_SOURCE = (file: string) =>
	/\.(ts|tsx|js|mjs)$/.test(file) &&
	!file.startsWith('vendor/') &&
	!file.startsWith('src/lib/generated/');
const UVX_CALL = /(?:^\s*|[=(]\s*|\$\(\s*|&&\s*|;\s*)uvx\s+(?!--version\b)/;
const PROFILE_WRITE =
	/(?:>>?|\btee(?:\s+-a)?)\s*"?(?:\$HOME|\$\{HOME\}|~)\/\.(?:zshrc|zprofile|bashrc|bash_profile|profile)\b/;
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
		appliesTo: PURE_CORE,
		comment: /^\s*\/\//,
		findIn: (line) => CODE_ONLY(line).match(/\b(?:std::(?:fs|net|process)|tokio)::/)?.[0] ?? null,
		message:
			'does I/O in a pure core. Fix: take the bytes, paths, or facts as input and let abb-engine do the I/O (crates/AGENTS.md).',
	},
	{
		appliesTo: (file) => PURE_CORE(file) && file.startsWith('crates/abb-remote-source-core/'),
		comment: /^\s*\/\//,
		findIn: (line) => CODE_ONLY(line).match(/audible|indexer|prowlarr/i)?.[0] ?? null,
		message:
			'names a provider in the provider-neutral core. Fix: put provider rules in abb-audible-core or the engine provider module (crates/AGENTS.md).',
	},
	{
		appliesTo: (file) => SOLID_SOURCE(file) && file !== 'src/ui/foundation/CoverImage.tsx',
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) => line.match(/<img\b/)?.[0] ?? null,
		message:
			'renders a cover outside CoverImage, which owns lazy loading, the fade-in, and the broken-image state. Fix: render `<CoverImage>` from src/ui/foundation.',
	},
	{
		appliesTo: SCRIPT_SOURCE,
		findIn: (line) =>
			/\boxlint-disable/.test(line) && !line.includes(' -- ') ? 'oxlint-disable' : null,
		message:
			'suppresses a lint without a reason. Fix: name the rule and give its reason after ` -- ` (scripts/AGENTS.md).',
	},
	{
		appliesTo: OWNED_SHELL,
		comment: /^\s*#/,
		findIn: (line) => (UVX_CALL.test(line) && !line.includes('--exclude-newer') ? 'uvx' : null),
		message:
			'runs a Python tool without the release wait. Fix: call `uvx --exclude-newer "10 days"` (scripts/AGENTS.md, Dependencies).',
	},
	{
		appliesTo: OWNED_SHELL,
		comment: /^\s*#/,
		findIn: (line) => line.match(PROFILE_WRITE)?.[0] ?? null,
		message:
			'writes a shell profile; setup never edits profiles. Fix: print the line for the owner, or export it through `setup.sh --print-env`.',
	},
	{
		appliesTo: ENGINE_DIR('metadata'),
		comment: /^\s*\/\//,
		findIn: (line) => line.match(/\bcrate::audio::(?:\{[^}]*)?\bAudioFile\b/)?.[0] ?? null,
		message:
			'reads the audio import model from metadata. Fix: pass the paths or tag facts metadata needs (crates/abb-engine/src/metadata/AGENTS.md).',
	},
	{
		appliesTo: (file) => PROVIDER_NEUTRAL_OWNERS.some((dir) => ENGINE_DIR(dir)(file)),
		comment: /^\s*\/\//,
		findIn: (line) => line.match(/\bcrate::remote_source\b/)?.[0] ?? null,
		message:
			'reaches into remote_source from an owner that takes provider-neutral facts. Fix: receive the fact through its caller (crates/abb-engine/src/remote_source/AGENTS.md).',
	},
	{
		appliesTo: ENGINE_DIR('work_runtime'),
		comment: /^\s*\/\//,
		findIn: (line) => line.match(/\bremote_source::materializer\b/)?.[0] ?? null,
		message:
			'imports the materializer into WorkRuntime. Fix: reach processing through `processing::run` (crates/abb-engine/src/work_runtime/AGENTS.md).',
	},
	{
		appliesTo: SOLID_SOURCE,
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) => CODE_ONLY(line).match(DESTRUCTURED_PROPS)?.[0] ?? null,
		message:
			'destructures Solid component props, which reads each prop once and drops its reactivity. Fix: read `props.name`, or split props with `omit`.',
	},
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

function testModuleFindings(file: string, lines: string[]): Finding[] {
	if (!OWNED_RUST(file) || RUST_TEST_FILE(file)) return [];
	const base = path.basename(file, '.rs');
	const folder = path.basename(path.dirname(file));
	const allowed =
		base === 'mod'
			? [`${folder}_tests.rs`, 'tests.rs']
			: folder === 'bin'
				? [`${base}/tests.rs`]
				: [`${base}_tests.rs`];
	const expected = allowed[0];
	return lines.flatMap((line, index) => {
		if (!/^\s*mod tests;/.test(line)) return [];
		const declared = lines[index - 1]?.match(/#\[path = "([^"]+)"\]/)?.[1] ?? 'tests.rs';
		if (allowed.includes(declared)) return [];
		return [
			{
				file,
				line: index + 1,
				message: `\`mod tests;\` loads ${declared}, not the sibling ${expected}. Fix: \`#[cfg(test)] #[path = "${expected}"] mod tests;\` (AGENTS.md, Testing And Proof).`,
			},
		];
	});
}

export function checkFile(file: string, text: string): Finding[] {
	const rules = RULES.filter((rule) => rule.appliesTo(file));
	const findings: Finding[] = testModuleFindings(file, text.split('\n'));
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
		.filter((file) => file && (OWNED_RUST(file) || RULES.some((rule) => rule.appliesTo(file))));
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
