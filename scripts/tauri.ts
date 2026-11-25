import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

interface DevelopmentIdentity {
	identifier: string;
	productName: string;
}

type GitRunner = (args: string[]) => { status: number | null; stdout: string };

export function developmentIdentity(repoRoot: string): DevelopmentIdentity {
	const checkout = realpathSync(repoRoot);
	const config: DevelopmentIdentity = JSON.parse(
		readFileSync(path.join(checkout, 'src-tauri/tauri.conf.json'), 'utf8'),
	);
	const checkoutId = createHash('sha256').update(checkout).digest('hex').slice(0, 12);
	return {
		identifier: `${config.identifier}.dev.${checkoutId}`,
		productName: `${config.productName} Dev ${checkoutId.slice(0, 6)}`,
	};
}

export function withDevelopmentIdentity(repoRoot: string, args: string[]): string[] {
	const boundary = args.indexOf('--');
	const cliArgs = boundary < 0 ? args : args.slice(0, boundary);
	const command = cliArgs.find((arg) => !arg.startsWith('-'));
	const debugBuild = command === 'build' && (cliArgs.includes('--debug') || cliArgs.includes('-d'));
	if (command !== 'dev' && !debugBuild) return args;

	return [
		...cliArgs,
		'--config',
		JSON.stringify(developmentIdentity(repoRoot)),
		...(boundary < 0 ? [] : args.slice(boundary)),
	];
}

const runGit: GitRunner = (args) => {
	const result = spawnSync('git', args, { encoding: 'utf8' });
	return { status: result.status, stdout: result.stdout ?? '' };
};

export function buildRevision(git: GitRunner = runGit): string {
	const head = git(['rev-parse', '--short=12', 'HEAD']);
	const status = git(['status', '--porcelain']);
	if (head.status !== 0 || status.status !== 0) return 'unknown';
	return status.stdout.trim() ? `${head.stdout.trim()}-dirty` : head.stdout.trim();
}

if (import.meta.main) {
	const args = withDevelopmentIdentity(process.cwd(), process.argv.slice(2));
	const revision = spawnSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' });
	const dirty = spawnSync('git', ['status', '--porcelain'], { encoding: 'utf8' });
	console.info(
		`build_checkout revision=${revision.status === 0 ? revision.stdout.trim() : 'unknown'} dirty=${dirty.status === 0 ? Boolean(dirty.stdout.trim()) : 'unknown'} checkout=${JSON.stringify(path.basename(realpathSync(process.cwd())))}`,
	);
	const tauriBin = path.join(import.meta.dir, '..', 'node_modules/.bin/tauri');
	process.on('SIGINT', () => {});
	const result = spawnSync(process.execPath, [tauriBin, ...args], {
		stdio: 'inherit',
		env: { ...process.env, ABB_BUILD_REVISION: buildRevision() },
	});
	if (result.error) console.error(result.error.message);
	process.exitCode = result.status ?? 1;
}
