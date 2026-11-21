#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

usage() {
	cat <<'EOF'
Usage: bash scripts/github-release.sh <plan|create> [staged-dir]

plan    Decide whether this push to main should build a release.
create  Tag vX.Y.Z when it is absent and publish the GitHub Release for the
        staged DMG and its SHA-256. A second run does not move the tag or
        publish a second release. Refuses unless this is a push to main.
EOF
}

if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
	usage
	exit 0
fi

command="${1:-}"
dir="${2:-}"
if [[ "$command" != plan && "$command" != create ]]; then
	usage >&2
	exit 1
fi
if [[ "$command" == create && -z "$dir" ]]; then
	usage >&2
	exit 1
fi
if [[ "$command" == create && ( "${GITHUB_EVENT_NAME:-}" != push || "${GITHUB_REF:-}" != refs/heads/main ) ]]; then
	printf '%s\n' 'Refusing to tag or publish outside a push to main. Fix: merge a pull request that bumps the version; that push publishes the release.' >&2
	exit 1
fi

repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set. Fix: run this step in GitHub Actions.}"
version="$(bun scripts/version-files.ts --print)"
tag="v${version}"
product="$(jq -r '.productName' src-tauri/tauri.conf.json)"
if [[ -z "$product" || "$product" == null ]]; then
	printf '%s\n' 'src-tauri/tauri.conf.json has no productName.' >&2
	exit 1
fi
asset="$(printf '%s_%s_aarch64.dmg' "$(printf '%s' "$product" | tr ' ' '.')" "$version")"
checksum="${asset}.sha256"
head="$(git rev-parse HEAD)"
head_lc="$(printf '%s' "$head" | tr '[:upper:]' '[:lower:]')"
err="$(mktemp)"
notes="$(mktemp)"
out="$(mktemp)"
trap 'rm -f "$err" "$notes" "$out"' EXIT

if [[ ! "$head" =~ ^[0-9a-fA-F]{40}$ ]]; then
	printf '%s\n' "HEAD is not a full commit SHA (${head}). Fix: check out the commit to release." >&2
	exit 1
fi
if [[ -n "${GITHUB_SHA:-}" && "$GITHUB_SHA" != "$head" ]]; then
	printf '%s\n' "Checkout ${head} is not the workflow commit ${GITHUB_SHA}. Fix: rerun the workflow so it builds the commit it tags." >&2
	exit 1
fi

tag_sha=""
if gh api "repos/${repo}/git/ref/tags/${tag}" --jq '{type:.object.type,sha:.object.sha}' >"$out" 2>"$err"; then
	ref_type="$(jq -r '.type' "$out")"
	tag_sha="$(jq -r '.sha' "$out")"
	if [[ "$ref_type" == tag ]]; then
		tag_sha="$(gh api "repos/${repo}/git/tags/${tag_sha}" --jq '.object.sha')"
	elif [[ "$ref_type" != commit ]]; then
		printf '%s\n' "Tag ${tag} points at a ${ref_type}, not a commit." >&2
		exit 1
	fi
elif ! grep -Eq '404|Not Found' "$err"; then
	cat "$err" >&2
	exit 1
fi
tag_lc="$(printf '%s' "$tag_sha" | tr '[:upper:]' '[:lower:]')"

read_release() {
	assets=""
	target=""
	if gh release view "$tag" --repo "$repo" --json isDraft,assets,targetCommitish >"$out" 2>"$err"; then
		if [[ "$(jq -r '.isDraft' "$out")" == true ]]; then
			release_state="draft"
		else
			release_state="published"
		fi
		assets="$(jq -r '.assets[].name' "$out")"
		target="$(jq -r '.targetCommitish // empty' "$out")"
	elif grep -Eqi 'not found' "$err"; then
		release_state="missing"
	else
		cat "$err" >&2
		exit 1
	fi
}

has_name() {
	printf '%s\n' "$assets" | grep -qxF -- "$1"
}

read_release
complete=false
if [[ "$release_state" == published ]] && has_name "$asset" && has_name "$checksum"; then
	complete=true
fi

action="build"
create_tag=false
if [[ -n "$tag_sha" && "$tag_lc" != "$head_lc" ]]; then
	action="skip"
	if [[ "$release_state" == published && "$complete" != true ]]; then
		printf '%s\n' "Tag ${tag} points at ${tag_sha}, not this commit, and the published release is missing assets. Not tagging or publishing. Fix: delete that release by hand, then rerun the workflow of ${tag_sha}."
	elif [[ "$complete" == true ]]; then
		printf '%s\n' "${tag} is already published from ${tag_sha}. Not building."
	else
		printf '%s\n' "Tag ${tag} points at ${tag_sha}, not ${head}. Not tagging or publishing. Rerun the workflow of ${tag_sha} if that release did not finish."
	fi
elif [[ "$complete" == true ]]; then
	action="skip"
	printf '%s\n' "${tag} is already published with its DMG and SHA-256. Not building."
elif [[ "$release_state" == published ]]; then
	missing=""
	has_name "$asset" || missing="$asset"
	has_name "$checksum" || missing="${missing:+$missing, }${checksum}"
	printf '%s\n' "Published release ${tag} is immutable and is missing ${missing}. Fix: delete that release by hand, then rerun. This workflow will not publish a second release." >&2
	exit 1
elif [[ "$release_state" == draft && -z "$tag_sha" ]]; then
	target_lc="$(printf '%s' "$target" | tr '[:upper:]' '[:lower:]')"
	if [[ "$target_lc" == "$head_lc" ]]; then
		printf '%s\n' "Draft ${tag} has no tag yet and targets this commit. Finishing that draft."
	else
		printf '%s\n' "A draft ${tag} exists without the tag and targets ${target:-nothing}, not ${head}. Fix: delete the draft, then rerun." >&2
		exit 1
	fi
elif [[ -z "$tag_sha" ]]; then
	create_tag=true
	printf '%s\n' "No tag ${tag}. Building the DMG, then tagging this commit and publishing."
else
	printf '%s\n' "Tag ${tag} is on this commit and the release is not published. Building the DMG and finishing the release."
fi

if [[ "$action" == build ]]; then
	bun scripts/version-files.ts --notes >/dev/null
fi
if [[ "$command" == plan ]]; then
	if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
		if [[ "$action" == build ]]; then
			printf 'publish=true\n' >>"$GITHUB_OUTPUT"
		else
			printf 'publish=false\n' >>"$GITHUB_OUTPUT"
		fi
	fi
	exit 0
fi
if [[ "$action" != build ]]; then
	exit 0
fi

shopt -s nullglob
staged_dmgs=("${dir%/}"/*.dmg)
shopt -u nullglob
if [[ "${#staged_dmgs[@]}" -ne 1 ]]; then
	printf '%s\n' "Expected one staged DMG in ${dir}, found ${#staged_dmgs[@]}. Fix: stage the arm64 DMG ${asset}." >&2
	exit 1
fi
dmg="${staged_dmgs[0]}"
staged_name="$(basename "$dmg")"
if [[ "$staged_name" != "$asset" ]]; then
	printf '%s\n' "Staged DMG ${staged_name} is not ${asset}. The publish runner is macos-26, which is arm64. Fix: stage that asset." >&2
	exit 1
fi
sum="${dmg}.sha256"
if [[ ! -f "$sum" ]]; then
	printf '%s\n' "Staged assets ${asset} and ${checksum} are missing in ${dir}. Fix: build and stage the DMG before publishing." >&2
	exit 1
fi
recorded="$(awk 'NR==1 { print $1; exit }' "$sum")"
actual="$(shasum -a 256 "$dmg" | awk '{ print $1 }')"
if [[ "$recorded" != "$actual" ]]; then
	printf '%s\n' "Staged checksum ${recorded} does not match the DMG ${actual}. Fix: stage the DMG again; the published file must be the one that was checked." >&2
	exit 1
fi
printf 'DMG %s\n' "$asset"
printf 'SHA-256 %s\n' "$actual"
bun scripts/version-files.ts --notes >"$notes"
title="AudioBook Boss ${tag}"
if [[ "$release_state" == missing ]]; then
	if [[ "$create_tag" == true ]]; then
		gh release create "$tag" "$dmg" "$sum" --repo "$repo" --draft --target "$head" --title "$title" --notes-file "$notes"
	else
		gh release create "$tag" "$dmg" "$sum" --repo "$repo" --draft --verify-tag --title "$title" --notes-file "$notes"
	fi
	read_release
fi
if [[ "$release_state" == missing ]]; then
	printf '%s\n' "GitHub Release ${tag} was not created. Fix: rerun this workflow." >&2
	exit 1
fi
if [[ "$release_state" == draft ]]; then
	missing_files=()
	if ! has_name "$asset"; then
		missing_files+=("$dmg")
	fi
	if ! has_name "$checksum"; then
		missing_files+=("$sum")
	fi
	if [[ "${#missing_files[@]}" -gt 0 ]]; then
		gh release upload "$tag" "${missing_files[@]}" --repo "$repo"
		read_release
	fi
	if ! has_name "$asset" || ! has_name "$checksum"; then
		printf '%s\n' "Draft ${tag} is missing ${asset} or ${checksum}. Fix: rerun this workflow; it will not create a second release." >&2
		exit 1
	fi
	gh release edit "$tag" --repo "$repo" --draft=false --title "$title" --notes-file "$notes"
	read_release
fi
if [[ "$release_state" != published ]] || ! has_name "$asset" || ! has_name "$checksum"; then
	printf '%s\n' "GitHub Release ${tag} is not published with its DMG and SHA-256. Fix: rerun this workflow; it will not create a second release." >&2
	exit 1
fi
gh release view "$tag" --repo "$repo" --json url --jq '.url'
