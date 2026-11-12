import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';

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
const FAAC_AND_FAAD_ADAPTERS = new Set([
	'crates/abb-engine/src/audio/processor/encoder/faac.rs',
	'crates/abb-engine/src/audio/processor/faad_decoder.rs',
]);
const ENGINE_OUTSIDE_CODEC_ADAPTERS = (file: string) =>
	OWNED_RUST(file) &&
	!RUST_TEST_FILE(file) &&
	file.startsWith('crates/abb-engine/src/') &&
	!FAAC_AND_FAAD_ADAPTERS.has(file);
const SOLID_SOURCE = (file: string) => file.startsWith('src/') && file.endsWith('.tsx');
const DESTRUCTURED_PROPS =
	/\bfunction [A-Z]\w*\s*\(\s*\{|\b(?:const|let) [A-Z]\w*\s*=\s*\(\s*\{|\b(?:const|let)\s*\{[^}]*\}\s*=\s*props\b/;
const ENGINE_DIR = (dir: string) => (file: string) =>
	OWNED_RUST(file) && !RUST_TEST_FILE(file) && file.startsWith(`crates/abb-engine/src/${dir}/`);
const PURE_CORE = (file: string) =>
	OWNED_RUST(file) && !RUST_TEST_FILE(file) && /^crates\/abb-[a-z-]+-core\/src\//.test(file);
const PROVIDER_NEUTRAL_OWNERS = [
	'processing',
	'audio',
	'metadata',
	'output_artifact',
	'work_runtime',
];
const PROCESSING_SOURCE = (file: string) =>
	file === 'crates/abb-engine/src/processing.rs' || ENGINE_DIR('processing')(file);
const FRONTEND_SOURCE = (file: string) =>
	/^src\/.*\.tsx?$/.test(file) &&
	!/\.test\.tsx?$/.test(file) &&
	!file.startsWith('src/test/') &&
	!file.startsWith('src/lib/generated/');
const FRONTEND_TEST = (file: string) => /^src\/.*\.test\.tsx?$/.test(file);
const FFMPEG_LAYER = (file: string) =>
	OWNED_RUST(file) && !RUST_TEST_FILE(file) && file.startsWith('crates/abb-ffmpeg/src/');
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
const HISTORY_COMMENT =
	/(?:^|\s)(?:\/\/+[!/]?|#|\/\*+|\*)\s.*\b(as it did|did under|this branch|this PR|formerly|dropped because|used to be)\b|(?:^|\s)(?:\/\/+|#)\s.*\b([0-9a-f]{8}:[\w./-]+:\d+)/;
const EXCUSE_COMMENT =
	/^\s*\/\/+[!/]?\s*(shortcut|TODO|FIXME|HACK|XXX|workaround)\b|^\s*\/\/.*(intentional architecture|not (?:a )?temporary)/;
const PLANNING_COMMENT = /^\s*\/\/.*(\(#\d+ (?:Branch|residual|follow-?up|phase))/;
const UNENFORCED_CLAIM = /^\s*\/\/.*\b(never panics?|cannot panic|will not panic)\b/;
const PIN_TAG = /^\s*#\s*pin: .+; unpin when /;

const SILENCED = /2>\s*\/dev\/null|\|\|\s*true\b/;
const CAPTURE_WITH_SILENCE = /\$\((?:[^()]|\([^()]*\))*(2>\s*\/dev\/null|\|\|\s*true\b)/;

const RULES: Rule[] = [
	{
		appliesTo: (file) =>
			OWNED_RUST(file) &&
			!RUST_TEST_FILE(file) &&
			file !== 'crates/abb-engine/src/audio/output_header.rs',
		comment: /^\s*\/\//,
		findIn: (line) => CODE_ONLY(line).match(/\.write_header(?:_with)?\(/)?.[0] ?? null,
		message:
			'writes an output header outside its owner, which pins the MP4 movie timescale so long chapters survive. Fix: call `crate::audio::write_output_header`.',
	},
	{
		appliesTo: PURE_CORE,
		comment: /^\s*\/\//,
		findIn: (line) => CODE_ONLY(line).match(/\b(?:std::(?:fs|net|process)|tokio)::/)?.[0] ?? null,
		message:
			'does I/O in a pure core. Fix: take the bytes, paths, or facts as input and let abb-engine do the I/O.',
	},
	{
		appliesTo: (file) => PURE_CORE(file) && file.startsWith('crates/abb-remote-source-core/'),
		comment: /^\s*\/\//,
		findIn: (line) => CODE_ONLY(line).match(/audible|indexer|prowlarr/i)?.[0] ?? null,
		message:
			'names a provider in the provider-neutral core. Fix: put provider rules in abb-audible-core or the engine provider module.',
	},
	{
		appliesTo: (file) => SOLID_SOURCE(file) && file !== 'src/ui/foundation/CoverImage.tsx',
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) => line.match(/<img\b/)?.[0] ?? null,
		message:
			'renders a cover outside CoverImage, which owns lazy loading, the fade-in, and the broken-image state. Fix: render `<CoverImage>` from src/ui/foundation.',
	},
	{
		appliesTo: (file) =>
			/^src\/.*\.tsx?$/.test(file) &&
			!/\.test\.tsx?$/.test(file) &&
			!file.startsWith('src/lib/platform/'),
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) => line.match(/navigator\.userAgent|⌘Q/)?.[0] ?? null,
		message:
			'words the host OS outside src/lib/platform. Fix: import the helper or constant from src/lib/platform (src/ui/AGENTS.md).',
	},
	{
		appliesTo: SCRIPT_SOURCE,
		findIn: (line) =>
			/\boxlint-disable/.test(line) && !line.includes(' -- ') ? 'oxlint-disable' : null,
		message:
			'suppresses a lint without a reason. Fix: name the rule and give its reason after ` -- `.',
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
			'reads the audio import model from metadata. Fix: pass the paths or tag facts metadata needs (`PassthroughSource`).',
	},
	{
		appliesTo: ENGINE_DIR('audio'),
		comment: /^\s*\/\//,
		findIn: (line) => line.match(/\bcrate::processing\b/)?.[0] ?? null,
		message:
			'reaches up into processing from the audio pipeline. Fix: import the leaf name from crate::execution, which processing only re-exports.',
	},
	{
		appliesTo: (file) => PROVIDER_NEUTRAL_OWNERS.some((dir) => ENGINE_DIR(dir)(file)),
		comment: /^\s*\/\//,
		findIn: (line) => line.match(/\bcrate::remote_source\b/)?.[0] ?? null,
		message:
			'reaches into remote_source from an owner that takes provider-neutral facts. Fix: receive the provider-neutral fact through its caller, so provider rules stay in remote_source.',
	},
	{
		appliesTo: ENGINE_DIR('audio'),
		comment: /^\s*\/\//,
		findIn: (line) => CODE_ONLY(line).match(/\bfs::(?:rename|copy|hard_link)\b/)?.[0] ?? null,
		message:
			'moves a file outside the output artifact boundary. Fix: stage locally and commit through crate::output_artifact, which publishes only complete content.',
	},
	{
		appliesTo: PROCESSING_SOURCE,
		comment: /^\s*\/\//,
		findIn: (line) =>
			line.match(/\bpub(?:\([^)]*\))?\s+use\s+crate::(?:\{[^}]*\b)?output_artifact\b/)?.[0] ?? null,
		message:
			're-exports output artifact types from processing, which gives one type two import paths. Fix: callers import it from `crate::output_artifact`.',
	},
	{
		appliesTo: FRONTEND_SOURCE,
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) =>
			line.match(
				/\.(?:isValid|sourceRemoved)\b|\{[^}]*\b(?:isValid|sourceRemoved)\b[^}]*\}\s*=[^=]/,
			)?.[0] ?? null,
		message:
			'reads a source flag the engine already folds into `status`. Fix: branch on the title or source `status` (`ready`, `failed`, `removedFinished`).',
	},
	{
		appliesTo: FRONTEND_TEST,
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) =>
			line.match(/\bgetElementsByClassName\b|\.classList\b|\.className\b|\btoHaveClass\(/)?.[0] ??
			null,
		message:
			'reads CSS classes in a test, so a style refactor breaks it. Fix: find the element by role and accessible name, and read a `data-state` or `data-status` attribute.',
	},
	{
		appliesTo: (file) =>
			ENGINE_DIR('session')(file) &&
			!file.endsWith('/runtime.rs') &&
			!file.startsWith('crates/abb-engine/src/session/runtime/'),
		comment: /^\s*\/\//,
		findIn: (line) =>
			CODE_ONLY(line).match(/\b(?:std|tokio)::fs\b|\bFile::(?:open|create)\b/)?.[0] ?? null,
		message:
			'does file I/O in a session state module. Fix: return a ticket (`ReadTicket`, `SavePlan`, `PlanTicket`) and let `runtime.rs` or `runtime/` perform the I/O.',
	},
	{
		appliesTo: (file) => file === 'src/test/fixtures/fakeEngine.ts',
		comment: /^\s*(\/\/|\*|\/\*)/,
		findIn: (line) =>
			line.match(
				/\bintent\.kind\b(?!\s*===\s*'import')|\.(?:titleTotals|totalSizeBytes|canGroup)\s*=(?!=)/,
			)?.[0] ?? null,
		message:
			'puts a product rule in the fake engine. Fix: seed the answer on the intent, or add a recorded scenario (src/app/AGENTS.md, Tests).',
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
		appliesTo: ENGINE_OUTSIDE_CODEC_ADAPTERS,
		comment: /^\s*\/\//,
		findIn: (line) =>
			CODE_ONLY(line).match(/\bunsafe\s*(\{|fn\b|impl\b|trait\b|extern\b)/)?.[0] ?? null,
		message:
			'is unsafe in the engine. Fix: call `abb_ffmpeg` for FFmpeg work and add a missing operation there; only the FAAC and FAAD3 adapters hold `unsafe`.',
	},
	{
		appliesTo: OWNED_SOURCE,
		findIn: (line) => {
			const match = line.match(HISTORY_COMMENT);
			return match ? (match[1] ?? match[2]) : null;
		},
		message:
			"is a comment that narrates history. Fix: delete it and state today's rule as a name, type, test, or check.",
	},
	{
		appliesTo: OWNED_RUST,
		findIn: (line) => line.match(EXCUSE_COMMENT)?.[1] ?? line.match(EXCUSE_COMMENT)?.[2] ?? null,
		message:
			'is a comment that excuses code. Fix: fix the code, or record the gap in an issue; do not leave it in code.',
	},
	{
		appliesTo: OWNED_RUST,
		findIn: (line) => line.match(PLANNING_COMMENT)?.[1] ?? null,
		message:
			'is a comment that cites planning jargon. Fix: state the rule; keep at most an issue number.',
	},
	{
		appliesTo: OWNED_RUST,
		findIn: (line) => line.match(/^\s*\/\/\s*(Safety:)/)?.[1] ?? null,
		message:
			'looks like a safety comment but is not one. Fix: write `// SAFETY:` directly above an `unsafe` block, or delete the comment.',
	},
	{
		appliesTo: OWNED_RUST,
		findIn: (line) => line.match(UNENFORCED_CLAIM)?.[1] ?? null,
		message:
			'is a comment that claims what nothing enforces. Fix: return a `Result`, assert it, or drop the claim.',
	},
	{
		appliesTo: OWNED_RUST,
		findIn: (line) => line.match(/^\s*\/\/\/.*(\(None if [a-z ]+\))/)?.[1] ?? null,
		message:
			'restates the `Option` in its doc, and the doc reaches the generated TypeScript. Fix: state the unit or meaning only.',
	},
	{
		appliesTo: OWNED_RUST,
		findIn: (line) => line.match(/reason\s*=\s*"(joined by (?:the|a) )/)?.[1] ?? null,
		message:
			'gives a vague join reason. Fix: name the function that joins the task, as `joined by <function>`.',
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

const SELECTOR_CALL =
	/\b(querySelector(?:All)?|closest|matches)\s*(?:<[^>]*>)?\(\s*$|\b(querySelector(?:All)?|closest|matches)\s*(?:<[^>]*>)?\(\s*(['"`])(.*?)\3/;

function classSelectorFindings(file: string, lines: string[]): Finding[] {
	if (!FRONTEND_TEST(file)) return [];
	return lines.flatMap((line, index) => {
		const call = line.match(SELECTOR_CALL);
		if (!call) return [];
		const selector = call[4] ?? lines[index + 1]?.match(/^\s*(['"`])(.*?)\1/)?.[2] ?? '';
		if (!/\.[A-Za-z_-]/.test(selector.replace(/\[[^\]]*\]/g, ''))) return [];
		return [
			{
				file,
				line: index + 1,
				message: `\`${selector}\` selects by CSS class in a test, so a style refactor breaks it. Fix: find the element by role and accessible name, and read a \`data-state\` or \`data-status\` attribute.`,
			},
		];
	});
}

const PUBLIC_ITEM =
	/^\s*pub\s+(?:(?:const|unsafe|extern\s+"C")\s+)*(?:fn|type)\b|^\s*pub\s+\w+\s*:/;

function ffmpegPointerFindings(file: string, lines: string[]): Finding[] {
	if (!FFMPEG_LAYER(file)) return [];
	return lines.flatMap((line, index) => {
		if (!PUBLIC_ITEM.test(line)) return [];
		let end = index;
		if (/\b(?:fn|type)\b/.test(line)) {
			while (end < lines.length - 1 && !/[{;]\s*$/.test(lines[end])) end += 1;
		}
		const signature = lines
			.slice(index, end + 1)
			.join(' ')
			.split('{')[0];
		if (!/\*\s*(?:const|mut)\b/.test(signature)) return [];
		return [
			{
				file,
				line: index + 1,
				message:
					'puts a raw pointer in the public abb-ffmpeg surface, so the engine would hold an FFmpeg lifetime. Fix: take and return owned values or a borrowing wrapper; keep pointers `pub(crate)`.',
			},
		];
	});
}

export function cargoPinFindings(file: string, text: string): Finding[] {
	if (path.basename(file) !== 'Cargo.toml' || file.startsWith('vendor/')) return [];
	const lines = text.split('\n');
	return lines.flatMap((line, index) => {
		if (!/"=\d/.test(line) || /^\s*#/.test(line)) return [];
		for (let above = index - 1; above >= 0 && lines[above].trim() !== ''; above -= 1) {
			if (PIN_TAG.test(lines[above])) return [];
		}
		return [
			{
				file,
				line: index + 1,
				message: `\`${line.trim()}\` is an exact pin without a tag. Fix: add \`# pin: <why>; unpin when <trigger or YYYY-MM-DD>\` above it (scripts/AGENTS.md, Dependencies).`,
			},
		];
	});
}

export function packagePinFindings(packageJson: string, scriptsGuidance: string): Finding[] {
	const manifest = JSON.parse(packageJson) as {
		dependencies?: Record<string, string>;
		devDependencies?: Record<string, string>;
		overrides?: Record<string, string>;
	};
	const exact = Object.entries({ ...manifest.dependencies, ...manifest.devDependencies })
		.filter(([, version]) => /^(?:npm:[^@]+@)?\d/.test(version))
		.map(([name]) => name);
	const overrides = Object.keys(manifest.overrides ?? {}).map((name) => `${name} (override)`);
	return [...exact, ...overrides]
		.filter((pin) => !scriptsGuidance.includes(`| \`package.json\` | \`${pin}\` |`))
		.map((pin) => ({
			file: 'package.json',
			line: 1,
			message: `\`${pin}\` is an exact pin without a row in the pin table. Fix: add \`| \`package.json\` | \`${pin}\` | pin: <why>; unpin when <trigger> |\` to scripts/AGENTS.md, Dependencies.`,
		}));
}

export function checkFile(file: string, text: string): Finding[] {
	const rules = RULES.filter((rule) => rule.appliesTo(file));
	const findings: Finding[] = [
		...testModuleFindings(file, text.split('\n')),
		...classSelectorFindings(file, text.split('\n')),
		...ffmpegPointerFindings(file, text.split('\n')),
		...cargoPinFindings(file, text),
	];
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

const SAMPLES_DIR = 'crates/abb-engine/src/app_settings/saved_settings/';
const CHANGE_VERB: Record<string, string> = {
	M: 'modified',
	D: 'deleted',
	R: 'renamed',
	T: 'changed',
};

export function frozenSampleFindings(nameStatus: string): Finding[] {
	return nameStatus.split('\n').flatMap((entry) => {
		const [status, file] = entry.split('\t');
		if (!file?.startsWith(SAMPLES_DIR) || status.startsWith('A')) return [];
		const verb = CHANGE_VERB[status[0]] ?? 'changed';
		return [
			{
				file,
				line: 1,
				message: `was ${verb}. Samples are frozen saved-settings files from real releases. Fix: restore it and add a new sample instead, so every release's saved file keeps loading.`,
			},
		];
	});
}

function git(repoRoot: string, args: string[]): string {
	return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' });
}

export function changedSampleFindings(repoRoot: string, against?: string): Finding[] {
	if (against === undefined) {
		return frozenSampleFindings(
			git(repoRoot, ['diff', '--cached', '--find-renames', '--name-status']),
		);
	}
	const resolved = git(repoRoot, [
		'rev-parse',
		'--verify',
		'--end-of-options',
		`${against}^{commit}`,
	]).trim();
	const mergeBase = git(repoRoot, ['merge-base', '--', resolved, 'HEAD']).trim();
	return frozenSampleFindings(
		git(repoRoot, ['diff', '--find-renames', '--name-status', mergeBase, 'HEAD']),
	);
}

function sampleFindings(repoRoot: string, against?: string): Finding[] {
	try {
		return changedSampleFindings(repoRoot, against);
	} catch (error) {
		throw new Error(
			`check-known-mistakes: cannot compare with ${against ?? 'the staged index'}. Fetch the base with its ancestry; a missing base fails closed.`,
			{ cause: error },
		);
	}
}

function main(): number {
	const { values } = parseArgs({ options: { against: { type: 'string' } } });
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const tracked = execFileSync('git', ['ls-files'], { cwd: repoRoot, encoding: 'utf8' })
		.split('\n')
		.filter(
			(file) =>
				file &&
				(OWNED_RUST(file) ||
					path.basename(file) === 'Cargo.toml' ||
					RULES.some((rule) => rule.appliesTo(file))),
		);
	const findings = [
		...tracked.flatMap((file) => checkFile(file, readFileSync(path.join(repoRoot, file), 'utf8'))),
		...packagePinFindings(
			readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
			readFileSync(path.join(repoRoot, 'scripts/AGENTS.md'), 'utf8'),
		),
		...sampleFindings(repoRoot, values.against),
	];
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
