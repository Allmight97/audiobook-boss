import { spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import {
	chmodSync,
	copyFileSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	renameSync,
	rmSync,
	writeFileSync,
	readdirSync,
	unlinkSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const aaxcleanHelperBaseName = 'abb-aaxclean-helper';

export function aaxcleanHelperPublishTarget(
	platform: NodeJS.Platform = process.platform,
	arch: string = process.arch,
): { rid: string; triple: string } {
	if (platform === 'darwin' && arch === 'arm64') {
		return { rid: 'osx-arm64', triple: 'aarch64-apple-darwin' };
	}
	if (platform === 'linux' && arch === 'x64') {
		return { rid: 'linux-x64', triple: 'x86_64-unknown-linux-gnu' };
	}
	if (platform === 'linux' && arch === 'arm64') {
		return { rid: 'linux-arm64', triple: 'aarch64-unknown-linux-gnu' };
	}
	throw new Error(
		`AAXClean helper publish is not supported on ${platform}/${arch}. Use macOS Apple Silicon or Linux x64/arm64.`,
	);
}

export const aaxcleanHelperTargetTriple = aaxcleanHelperPublishTarget().triple;

interface AaxcleanHelperPaths {
	projectPath: string;
	publishDir: string;
	publishedExecutablePath: string;
	sidecarDir: string;
	sidecarPath: string;
	rid: string;
	triple: string;
}

interface PublishAaxcleanHelperOptions {
	commandRunner?: typeof spawnSync;
	force?: boolean;
}

export function resolveDotnetCommand(): string {
	if (process.env.DOTNET_CLI && process.env.DOTNET_CLI.length > 0) {
		return process.env.DOTNET_CLI;
	}

	const userLocalDotnet = path.join(os.homedir(), '.dotnet', 'dotnet');
	if (existsSync(userLocalDotnet)) {
		return userLocalDotnet;
	}

	return 'dotnet';
}

export function resolveAaxcleanHelperPaths(repoRoot: string): AaxcleanHelperPaths {
	const { rid, triple } = aaxcleanHelperPublishTarget();
	const projectPath = path.join(
		repoRoot,
		'tools/abb-aaxclean-helper/src/AbbAaxcleanHelper/AbbAaxcleanHelper.csproj',
	);
	const publishDir = path.join(repoRoot, 'tools/abb-aaxclean-helper/.publish', rid);
	const sidecarDir = path.join(repoRoot, 'src-tauri/binaries');
	return {
		projectPath,
		publishDir,
		publishedExecutablePath: path.join(publishDir, aaxcleanHelperBaseName),
		sidecarDir,
		sidecarPath: path.join(sidecarDir, `${aaxcleanHelperBaseName}-${triple}`),
		rid,
		triple,
	};
}

export function publishAaxcleanHelper(
	repoRoot: string,
	options: PublishAaxcleanHelperOptions = {},
): string {
	const paths = resolveAaxcleanHelperPaths(repoRoot);
	const runner = options.commandRunner ?? spawnSync;
	const dotnet = resolveDotnetCommand();
	const sdk = runner(dotnet, ['--version'], {
		cwd: path.dirname(paths.projectPath),
		encoding: 'utf8',
	});
	const sdkVersion = sdk.stdout?.toString().trim();
	if (sdk.status !== 0 || !sdkVersion)
		throw new Error('Could not resolve the helper .NET SDK; run bash scripts/setup.sh rust');
	const identity = helperSourceIdentity(repoRoot, paths.rid, sdkVersion);
	if (!options.force && helperSidecarIsFresh(paths.sidecarPath, identity)) {
		console.log(`[aaxclean-helper] Using existing ${paths.sidecarPath}`);
		return paths.sidecarPath;
	}

	if (existsSync(paths.publishedExecutablePath)) {
		unlinkSync(paths.publishedExecutablePath);
	}

	const buildDirectory = mkdtempSync(path.join(os.tmpdir(), 'abb-helper-build-'));
	try {
		const result = runner(
			dotnet,
			[
				'publish',
				paths.projectPath,
				'-c',
				'Release',
				'-r',
				paths.rid,
				'-o',
				paths.publishDir,
				'--artifacts-path',
				buildDirectory,
			],
			{
				cwd: path.dirname(paths.projectPath),
				stdio: 'inherit',
			},
		);

		if (result.error) {
			throw result.error;
		}
		if (result.status !== 0) {
			const detail =
				typeof result.status === 'number'
					? `status ${result.status}`
					: result.signal
						? `signal ${result.signal}`
						: 'no successful exit status';
			throw new Error(`AAXClean helper publish failed (${detail})`);
		}
		if (!existsSync(paths.publishedExecutablePath)) {
			throw new Error(`Expected published AAXClean helper at ${paths.publishedExecutablePath}`);
		}

		if (identity !== helperSourceIdentity(repoRoot, paths.rid, sdkVersion)) {
			throw new Error('Helper source changed during publication; retry the build');
		}
		mkdirSync(paths.sidecarDir, { recursive: true });
		const next = `${paths.sidecarPath}.${randomUUID()}.tmp`;
		try {
			copyFileSync(paths.publishedExecutablePath, next);
			chmodSync(next, 0o755);
			renameSync(next, paths.sidecarPath);
		} finally {
			rmSync(next, { force: true });
		}
		writeFileSync(
			`${paths.sidecarPath}.inputs.json`,
			JSON.stringify({ inputs: identity, binary: digestFile(paths.sidecarPath) }),
		);
		console.log(`[aaxclean-helper] Published ${paths.sidecarPath}`);
		return paths.sidecarPath;
	} finally {
		rmSync(buildDirectory, { recursive: true, force: true });
	}
}

function digestFile(file: string): string {
	return createHash('sha256').update(readFileSync(file)).digest('hex');
}

function helperSidecarIsFresh(sidecarPath: string, identity: string): boolean {
	if (!existsSync(sidecarPath) || !existsSync(`${sidecarPath}.inputs.json`)) return false;
	try {
		const stamp = JSON.parse(readFileSync(`${sidecarPath}.inputs.json`, 'utf8')) as {
			inputs: string;
			binary: string;
		};
		return stamp.inputs === identity && stamp.binary === digestFile(sidecarPath);
	} catch {
		return false;
	}
}

function helperSourceIdentity(repoRoot: string, rid: string, sdk: string): string {
	const helperRoot = path.join(repoRoot, 'tools/abb-aaxclean-helper');
	const hash = createHash('sha256').update(JSON.stringify({ rid, sdk }));
	hash.update(readFileSync(new URL(import.meta.url)));
	for (const file of [...walk(helperRoot)].filter(helperSourceAffectsPublish).sort()) {
		hash.update(path.relative(helperRoot, file)).update('\0').update(readFileSync(file));
	}
	return hash.digest('hex');
}

function helperSourceAffectsPublish(filePath: string): boolean {
	return (
		filePath.endsWith('.cs') ||
		filePath.endsWith('.csproj') ||
		filePath.endsWith('.props') ||
		filePath.endsWith('.targets') ||
		path.basename(filePath) === 'global.json'
	);
}

function* walk(directory: string): Generator<string> {
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			if (entry.name === 'bin' || entry.name === 'obj' || entry.name === '.publish') {
				continue;
			}
			yield* walk(fullPath);
		} else {
			yield fullPath;
		}
	}
}

if (import.meta.main) {
	const repoRoot = path.resolve(import.meta.dir, '..');
	publishAaxcleanHelper(repoRoot);
}
