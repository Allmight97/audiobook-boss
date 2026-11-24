import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const SYS_CRATE = 'vendor/ffmpeg-sys-next-9.0.1';
const UPSTREAM_URL = 'https://github.com/FFmpeg/FFmpeg.git';
const CACHE_DIRECTORY = 'target/abb-ffmpeg-upstream';
const MIN_RELEASE_MAJOR = 9;

const USED_FILES = `${SYS_CRATE}/ffmpeg-used-files.txt`;
const RENAME_ROOTS = ['compat', 'libavcodec', 'libavformat', 'libavutil', 'libswresample'];

export interface Commit {
	hash: string;
	date: string;
	subject: string;
}

export interface Report {
	pin: string;
	head: string;
	commits: Commit[];
}

export type Gh = (args: string[], input?: string) => string;

export function dependencyPaths(text: string): string[] {
	return text
		.replace(/\\\r?\n/g, ' ')
		.split(/\s+/)
		.filter((word) => word.length > 0 && !word.endsWith(':') && !word.startsWith('/'))
		.map((word) => word.replace(/^\.\//, ''));
}

export function parseUsedFiles(text: string): string[] {
	return text.split('\n').filter((line) => line.length > 0);
}

export function renderUsedFiles(files: Iterable<string>): string {
	return `${[...new Set(files)].toSorted().join('\n')}\n`;
}

export function watchedPaths(used: string[], renames: string): string[] {
	const listed = new Set(used);
	const moved = renames
		.split('\n')
		.map((line) => line.split('\t'))
		.filter(([status, from, to]) => status?.startsWith('R') && to && listed.has(from))
		.map(([, , to]) => to);
	return [...new Set([...used, ...moved])].map((file) => `:(literal)${file}`);
}

export function parseCommits(log: string): Commit[] {
	return log
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => {
			const [hash = '', date = '', ...subject] = line.split('\t');
			return { hash, date, subject: subject.join('\t') };
		});
}

export function parseReleaseTags(refsText: string, minMajor = MIN_RELEASE_MAJOR): string[] {
	return [...refsText.matchAll(/^refs\/tags\/(n(\d+)\.(\d+)(?:\.(\d+))?)$/gm)]
		.map(([, tag, major, minor, patch]) => ({
			tag,
			version: [Number(major), Number(minor), Number(patch ?? 0)] as const,
		}))
		.filter(({ version: [major] }) => major >= minMajor)
		.toSorted(
			(a, b) =>
				b.version[0] - a.version[0] || b.version[1] - a.version[1] || b.version[2] - a.version[2],
		)
		.map(({ tag }) => tag);
}

export function releaseBranchForTag(tag: string): string {
	const match = /^n(\d+)\.(\d+)(?:\.\d+)?$/.exec(tag);
	if (!match) throw new Error(`Not an FFmpeg release tag: '${tag}'.`);
	return `release/${match[1]}.${match[2]}`;
}

export function findContainingRelease(pin: string, gh: Gh): string | null {
	const refs = gh([
		'api',
		'--paginate',
		'repos/FFmpeg/FFmpeg/git/matching-refs/tags/n',
		'--jq',
		'.[].ref',
	]);
	for (const tag of parseReleaseTags(refs)) {
		const status = gh([
			'api',
			`repos/FFmpeg/FFmpeg/compare/${pin}...${tag}?per_page=1`,
			'--jq',
			'.status',
		]);
		if (status === 'ahead' || status === 'identical') {
			return tag;
		}
	}
	return null;
}

export function releaseCheckComment(tag: string, pin: string): string {
	const branch = releaseBranchForTag(tag);
	return [
		`FFmpeg \`${tag}\` contains the pinned commit \`${pin.slice(0, 10)}\`, so \`${branch}\` is eligible to replace the \`master\` pin.`,
		`Next step: follow \`Moving the pin\` in \`vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md\` (export parity input first, then \`bun run ffmpeg:pin ${tag}\`), update or remove \`.github/workflows/ffmpeg-upstream.yml\`, and close this issue once landed.`,
	].join('\n\n');
}

export function syncReleaseCheck(pin: string, issueNumber: number, gh: Gh, slug: string): string {
	const state = gh(['api', `repos/${slug}/issues/${issueNumber}`, '--jq', '.state']);
	if (state !== 'open') {
		return `issue #${issueNumber} is ${state}; skipping release check`;
	}
	const tag = findContainingRelease(pin, gh);
	if (!tag) {
		return `no FFmpeg release tag contains pin ${pin.slice(0, 10)}`;
	}
	const branch = releaseBranchForTag(tag);
	const comments = gh([
		'api',
		'--paginate',
		`repos/${slug}/issues/${issueNumber}/comments?per_page=100`,
		'--jq',
		'.[] | {body: (.body // "")}',
	])
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => (JSON.parse(line) as { body: string }).body);
	if (comments.some((body) => body.includes(`\`${branch}\``))) {
		return `issue #${issueNumber} already notes ${branch}`;
	}
	gh(
		['api', '--method', 'POST', `repos/${slug}/issues/${issueNumber}/comments`, '--input', '-'],
		JSON.stringify({ body: releaseCheckComment(tag, pin) }),
	);
	return `commented on issue #${issueNumber} for ${tag} (${branch})`;
}

export function renderReport(report: Report): string {
	const lines = [
		'# FFmpeg upstream watch',
		'',
		`Pinned: \`${report.pin.slice(0, 10)}\`. Master head: \`${report.head.slice(0, 10)}\`.`,
		'',
	];
	if (report.commits.length === 0) {
		lines.push('no relevant upstream changes');
	} else {
		lines.push(
			`${report.commits.length} commit(s) after the pin touch sources ABB's build can compile:`,
			'',
			'| Commit | Date | Subject |',
			'| --- | --- | --- |',
			...report.commits.map(
				(commit) =>
					`| \`${commit.hash.slice(0, 10)}\` | ${commit.date} | ${commit.subject.replaceAll('|', '\\|')} |`,
			),
		);
	}
	return lines.join('\n');
}

function run(file: string, args: string[], cwd?: string, input?: string): string {
	return execFileSync(file, args, {
		cwd,
		input,
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
		stdio: [input === undefined ? 'ignore' : 'pipe', 'pipe', 'inherit'],
	}).trim();
}

function ensureClone(cache: string): void {
	if (existsSync(path.join(cache, 'HEAD'))) {
		run('git', ['fetch', '--quiet', 'origin', 'master:refs/heads/master'], cache);
		return;
	}
	run('git', [
		'clone',
		'--quiet',
		'--bare',
		'--filter=blob:none',
		'--single-branch',
		'--branch=master',
		UPSTREAM_URL,
		cache,
	]);
}

function repoSlug(): string {
	return (
		process.env.GITHUB_REPOSITORY ||
		run('gh', ['repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner'])
	);
}

function buildReport(repoRoot: string): Report {
	const pin = readFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), 'utf8').trim();
	const used = parseUsedFiles(readFileSync(path.join(repoRoot, USED_FILES), 'utf8'));
	const cache = path.join(repoRoot, CACHE_DIRECTORY, 'FFmpeg.git');
	ensureClone(cache);

	const renames = run(
		'git',
		[
			'diff',
			'--find-renames',
			'--name-status',
			'--diff-filter=R',
			pin,
			'master',
			'--',
			...RENAME_ROOTS,
		],
		cache,
	);
	const head = run('git', ['rev-parse', 'master'], cache);
	const commits = parseCommits(
		run(
			'git',
			['log', '--format=%H%x09%cs%x09%s', `${pin}..master`, '--', ...watchedPaths(used, renames)],
			cache,
		),
	);
	return { pin, head, commits };
}

function gitBlobHash(file: string): string {
	return run('git', ['hash-object', '--no-filters', file]);
}

function targetDirectory(repoRoot: string): string {
	return path.resolve(repoRoot, process.env.CARGO_TARGET_DIR ?? 'target');
}

function currentTrees(repoRoot: string, pin: string): string[] {
	const target = targetDirectory(repoRoot);
	const script = `build-script=${gitBlobHash(path.join(repoRoot, SYS_CRATE, 'build.rs'))}`;
	const profiles = readdirSync(target, { withFileTypes: true })
		.filter((entry) => entry.isDirectory())
		.flatMap((entry) => {
			const dir = path.join(target, entry.name);
			return existsSync(path.join(dir, 'build'))
				? [dir]
				: readdirSync(dir, { withFileTypes: true })
						.filter((child) => child.isDirectory())
						.map((child) => path.join(dir, child.name))
						.filter((child) => existsSync(path.join(child, 'build')));
		});
	const trees = new Set<string>();
	for (const profile of profiles) {
		for (const unit of readdirSync(path.join(profile, 'build'))) {
			const identity = path.join(profile, 'build', unit, 'out', 'abb-build-identity.txt');
			if (!unit.startsWith('ffmpeg-sys-next-') || !existsSync(identity)) continue;
			if (readFileSync(identity, 'utf8').split('\n')[0] !== script) continue;
			const tree = path.join(target, 'abb-ffmpeg-cache', gitBlobHash(identity));
			const stamp = path.join(tree, 'dist', 'abb-source-commit');
			if (existsSync(stamp) && readFileSync(stamp, 'utf8').startsWith(`${pin}:`)) {
				trees.add(tree);
			}
		}
	}
	return [...trees].toSorted();
}

function dependencyFiles(directory: string): string[] {
	return readdirSync(directory, { recursive: true, encoding: 'utf8' })
		.filter((file) => file.endsWith('.d'))
		.map((file) => path.join(directory, file));
}

function compiledFiles(tree: string): { used: Set<string>; tracked: Set<string> } {
	const source = path.join(tree, 'ffmpeg-9.0');
	const tracked = new Set(run('git', ['ls-files'], source).split('\n'));
	const used = new Set(
		dependencyFiles(source)
			.flatMap((file) => dependencyPaths(readFileSync(file, 'utf8')))
			.filter((file) => tracked.has(file)),
	);
	return { used, tracked };
}

export interface UsedFilesCheck {
	unlisted: string[];
	gone: string[];
}

export function compareUsedFiles(
	listed: string[],
	used: Set<string>,
	tracked: Set<string>,
): UsedFilesCheck {
	const names = new Set(listed);
	return {
		unlisted: [...used].filter((file) => !names.has(file)).toSorted(),
		gone: listed.filter((file) => !tracked.has(file)),
	};
}

function usedFiles(repoRoot: string, mode: string): string {
	const pin = readFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), 'utf8').trim();
	const trees = currentTrees(repoRoot, pin);
	if (trees.length === 0) {
		throw new Error(
			'No FFmpeg build of the current build.rs at the pin in this target directory. Build the engine first (bash scripts/verify.sh engine).',
		);
	}
	const file = path.join(repoRoot, USED_FILES);
	const listed = parseUsedFiles(readFileSync(file, 'utf8'));
	const used = new Set<string>();
	let tracked = new Set<string>();
	for (const tree of trees) {
		const compiled = compiledFiles(tree);
		for (const name of compiled.used) used.add(name);
		tracked = compiled.tracked;
	}
	const { unlisted, gone } = compareUsedFiles(listed, used, tracked);
	if (mode === 'update') {
		const kept = listed.filter((name) => tracked.has(name));
		writeFileSync(file, renderUsedFiles([...kept, ...used]));
		return `${USED_FILES}: added ${unlisted.length}, removed ${gone.length}.`;
	}
	if (unlisted.length === 0 && gone.length === 0) {
		return `${USED_FILES} covers the ${used.size} FFmpeg files this build compiles.`;
	}
	throw new Error(
		[
			`${USED_FILES} is out of date. Run bun run ffmpeg:used update on each platform that differs.`,
			...unlisted.map((name) => `  compiled, not listed: ${name}`),
			...gone.map((name) => `  listed, not in the pinned source: ${name}`),
		].join('\n'),
	);
}

export function withNoticeCommit(notice: string, pinned: string, commit: string): string {
	if (!notice.includes(pinned)) {
		throw new Error(`${SYS_CRATE}/ABB-PROVENANCE.md does not name the pinned commit ${pinned}.`);
	}
	return notice.replaceAll(pinned, commit);
}

export function movePin(repoRoot: string, commitish: string, gh: Gh): string {
	if (!/^(?:[0-9a-f]{7,40}|n\d+\.\d+(?:\.\d+)?)$/.test(commitish)) {
		throw new Error(
			`ffmpeg:pin needs an FFmpeg commit hash or release tag (nX.Y or nX.Y.Z), got '${commitish}'.`,
		);
	}
	const commit = gh(['api', `repos/FFmpeg/FFmpeg/commits/${commitish}`, '--jq', '.sha']);
	if (!/^[0-9a-f]{40}$/.test(commit)) {
		throw new Error(`Expected 40-character SHA for '${commitish}', got '${commit}'.`);
	}
	const revision = path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision');
	const notice = path.join(repoRoot, SYS_CRATE, 'ABB-PROVENANCE.md');
	const named = withNoticeCommit(
		readFileSync(notice, 'utf8'),
		readFileSync(revision, 'utf8').trim(),
		commit,
	);
	writeFileSync(revision, `${commit}\n`);
	writeFileSync(notice, named);
	return `Pinned FFmpeg ${commit}`;
}

if (import.meta.main) {
	try {
		const [flag, value, ...extra] = process.argv.slice(2);
		const repoRoot = path.resolve(import.meta.dirname, '..');
		const gh: Gh = (args, input) => run('gh', args, undefined, input);
		if (flag === '--pin' && value !== undefined && extra.length === 0) {
			console.log(movePin(repoRoot, value, gh));
		} else if (
			flag === '--used-files' &&
			(value === 'check' || value === 'update') &&
			extra.length === 0
		) {
			console.log(usedFiles(repoRoot, value));
		} else if (
			flag === '--release-check' &&
			value !== undefined &&
			/^\d+$/.test(value) &&
			extra.length === 0
		) {
			const pin = readFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), 'utf8').trim();
			console.log(syncReleaseCheck(pin, Number(value), gh, repoSlug()));
		} else if (flag === undefined) {
			console.log(renderReport(buildReport(repoRoot)));
		} else {
			throw new Error(`Unknown arguments: ${process.argv.slice(2).join(' ')}`);
		}
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
