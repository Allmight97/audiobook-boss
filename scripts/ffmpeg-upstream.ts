/**
 * What changed in FFmpeg master since ABB's pinned commit, in the sources
 * ABB's build can compile.
 *
 * Usage: bun run ffmpeg:upstream [--issue]
 *        bun run ffmpeg:pin <commit>
 *
 * Reads the pin from `vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision` and the
 * FFmpeg files ABB's build compiles from `ffmpeg-used-files.txt` beside it.
 * Fetches master into a blobless clone under `target/abb-ffmpeg-upstream/`
 * (reused across runs) and lists the commits after the pin that touch one of
 * those files, or the file it was renamed to. Prints a Markdown report. With `--issue` it instead keeps one open issue current through
 * `gh api` (REST only; cloud sessions block GraphQL): the issue exists while
 * relevant commits exist, and its body changes only when the pin or the
 * commit list changes. The scheduled workflow
 * `.github/workflows/ffmpeg-upstream.yml` runs it.
 *
 * `ffmpeg:used` (`--used-files check|update`) reads the compiler dependency
 * files of this checkout's current FFmpeg build trees: `check` fails when a
 * build compiled a file the list lacks or the list names a file the pinned
 * source no longer has; `update` rewrites the list to match.
 *
 * `ffmpeg:pin` (`--pin <commit>`) moves every pin of the FFmpeg commit at
 * once: the full hash into `ffmpeg-revision`, and `ffmpeg_cli_release` and
 * `ffmpeg_cli_build` in `scripts/setup.sh` to the newest FFmpeg-Builds master
 * autobuild of that commit. It changes nothing when no autobuild names the
 * commit, and lists the commits that have one.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const SYS_CRATE = 'vendor/ffmpeg-sys-next-9.0.1';
const UPSTREAM_URL = 'https://github.com/FFmpeg/FFmpeg.git';
export const ISSUE_TITLE = 'FFmpeg upstream: changes since the pin';
const NEXT_ACTION =
	'Next action: decide whether to move the pin (procedure: vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md, Moving the pin).';
const CACHE_DIRECTORY = 'target/abb-ffmpeg-upstream';
const SETUP_SH = 'scripts/setup.sh';
const AUTOBUILD_RELEASES = 'repos/FFmpeg-Builds/FFmpeg-Builds/releases?per_page=100';

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

/**
 * The repository paths a compiler dependency file (`make`'s `.d`) names:
 * every source and header after a target's colon, without absolute paths
 * (system and SDK headers) or a leading `./`.
 */
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

/** Each watched path, and the path a rename between the pin and master moved it to. */
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

export type Gh = (args: string[], input?: string) => string;

export interface OpenIssue {
	number: number;
	title: string;
	body: string;
}

export type IssueAction =
	| { kind: 'create'; body: string }
	| { kind: 'update'; number: number; body: string }
	| { kind: 'close'; number: number }
	| { kind: 'unchanged'; number: number }
	| { kind: 'none' };

export function issueBody(report: Report, checkedAt: Date): string {
	const stamp = checkedAt.toISOString().slice(0, 16).replace('T', ' ');
	return [
		`Checked ${stamp} UTC against pin ${report.pin.slice(0, 10)}.`,
		NEXT_ACTION,
		renderReport(report),
	].join('\n\n');
}

function listedState(body: string): { pin: string | undefined; commits: string[] } {
	return {
		pin: /^Checked .* against pin (\w+)\./.exec(body)?.[1],
		commits: [...body.matchAll(/^\| `(\w+)` \|/gm)].map(([, hash]) => hash),
	};
}

export function decideIssueAction(
	report: Report,
	issue: OpenIssue | undefined,
	checkedAt: Date,
): IssueAction {
	if (report.commits.length === 0) {
		return issue ? { kind: 'close', number: issue.number } : { kind: 'none' };
	}
	const body = issueBody(report, checkedAt);
	if (!issue) return { kind: 'create', body };
	const listed = listedState(issue.body);
	const current = listedState(body);
	const same =
		listed.pin === current.pin &&
		listed.commits.length === current.commits.length &&
		listed.commits.every((hash, index) => hash === current.commits[index]);
	return same
		? { kind: 'unchanged', number: issue.number }
		: { kind: 'update', number: issue.number, body };
}

export function syncIssue(report: Report, gh: Gh, slug: string, checkedAt: Date): string {
	const openIssues = gh([
		'api',
		'--paginate',
		`repos/${slug}/issues?state=open&per_page=100`,
		'--jq',
		'.[] | select(.pull_request == null) | {number, title, body: (.body // "")}',
	])
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => JSON.parse(line) as OpenIssue);
	const issue = openIssues.find((candidate) => candidate.title === ISSUE_TITLE);
	const action = decideIssueAction(report, issue, checkedAt);
	switch (action.kind) {
		case 'create': {
			const created = JSON.parse(
				gh(
					['api', '--method', 'POST', `repos/${slug}/issues`, '--input', '-'],
					JSON.stringify({ title: ISSUE_TITLE, body: action.body }),
				),
			) as { number: number };
			return `created issue #${created.number}`;
		}
		case 'update':
			gh(
				['api', '--method', 'PATCH', `repos/${slug}/issues/${action.number}`, '--input', '-'],
				JSON.stringify({ body: action.body }),
			);
			return `updated issue #${action.number}`;
		case 'close':
			gh(
				['api', '--method', 'PATCH', `repos/${slug}/issues/${action.number}`, '--input', '-'],
				JSON.stringify({ state: 'closed', state_reason: 'completed' }),
			);
			return `closed issue #${action.number}`;
		case 'unchanged':
			return `issue #${action.number} already lists these commits`;
		case 'none':
			return 'no relevant upstream changes and no open issue';
	}
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

/**
 * The FFmpeg source trees this checkout's current builds compiled: each build
 * script output whose identity names the current `build.rs`, mapped to the
 * cache tree build.rs keys by that identity's blob hash, at the pinned source.
 */
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

/** Every tracked FFmpeg file a tree's build compiled or included. */
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

export interface Release {
	tag_name: string;
	assets: string[];
}

export interface Autobuild {
	release: string;
	build: string;
}

/**
 * The master autobuilds setup can install, newest first: an `autobuild-`
 * release with the checksums and both Linux GPL archives of one describe id
 * (`N-<count>-g<short hash>`).
 */
export function masterAutobuilds(releases: Release[]): Autobuild[] {
	return releases.flatMap(({ tag_name: release, assets }) => {
		if (!release.startsWith('autobuild-') || !assets.includes('checksums.sha256')) return [];
		return assets.flatMap((asset) => {
			const build = /^ffmpeg-(N-\d+-g[0-9a-f]+)-linux64-gpl\.tar\.xz$/.exec(asset)?.[1];
			return build && assets.includes(`ffmpeg-${build}-linuxarm64-gpl.tar.xz`)
				? [{ release, build }]
				: [];
		});
	});
}

function shortHash(build: string): string {
	return build.slice(build.lastIndexOf('-g') + 2);
}

export function withCliPins(setupSh: string, autobuild: Autobuild): string {
	let text = setupSh;
	for (const [name, value] of [
		['ffmpeg_cli_release', autobuild.release],
		['ffmpeg_cli_build', autobuild.build],
	]) {
		const line = new RegExp(`^${name}=\\S*$`, 'm');
		if (!line.test(text)) throw new Error(`${SETUP_SH} has no ${name}= line.`);
		text = text.replace(line, `${name}=${value}`);
	}
	return text;
}

export function movePin(repoRoot: string, commitish: string, gh: Gh): string {
	if (!/^[0-9a-f]{7,40}$/.test(commitish)) {
		throw new Error(`ffmpeg:pin needs an FFmpeg commit hash, got '${commitish}'.`);
	}
	const commit = gh(['api', `repos/FFmpeg/FFmpeg/commits/${commitish}`, '--jq', '.sha']);
	const autobuilds = masterAutobuilds(
		gh([
			'api',
			'--paginate',
			AUTOBUILD_RELEASES,
			'--jq',
			'.[] | {tag_name, assets: [.assets[].name]}',
		])
			.split('\n')
			.filter((line) => line.length > 0)
			.map((line) => JSON.parse(line) as Release),
	);
	const autobuild = autobuilds.find(({ build }) => commit.startsWith(shortHash(build)));
	if (!autobuild) {
		const built = autobuilds.map(({ build }) => shortHash(build)).join(', ');
		throw new Error(
			`No FFmpeg-Builds master autobuild names ${commit.slice(0, 10)}. Commits that have one: ${built}.`,
		);
	}
	const setupSh = path.join(repoRoot, SETUP_SH);
	const pinned = withCliPins(readFileSync(setupSh, 'utf8'), autobuild);
	writeFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), `${commit}\n`);
	writeFileSync(setupSh, pinned);
	return `Pinned FFmpeg ${commit}: ffmpeg_cli_release=${autobuild.release} ffmpeg_cli_build=${autobuild.build}`;
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
		} else if (flag === undefined || (flag === '--issue' && value === undefined)) {
			const report = buildReport(repoRoot);
			console.log(
				flag === '--issue' ? syncIssue(report, gh, repoSlug(), new Date()) : renderReport(report),
			);
		} else {
			throw new Error(`Unknown arguments: ${process.argv.slice(2).join(' ')}`);
		}
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
