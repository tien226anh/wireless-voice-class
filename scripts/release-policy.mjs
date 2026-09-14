import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const versionPattern = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function versionParts(tag) {
  const match = versionPattern.exec(tag);
  if (!match || !match.slice(1).every(n => Number.isSafeInteger(Number(n)))) {
    throw new Error(`Invalid version ${JSON.stringify(tag)}; use vMAJOR.MINOR.PATCH, for example v0.5.1.`);
  }
  return match.slice(1).map(Number);
}

function compare(a, b) {
  const left = versionParts(a);
  const right = versionParts(b);
  for (let i = 0; i < 3; i++) {
    if (left[i] !== right[i]) return left[i] - right[i];
  }
  return 0;
}

function stableTags(tags) {
  return tags.filter(tag => versionPattern.test(tag)).sort(compare);
}

export function packageVersion(root = '.') {
  const manifest = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  const section = manifest.match(/^\[package\]\s*\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1];
  const version = section?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  versionParts(`v${version}`);
  return `v${version}`;
}

export function suggestVersion(tags, baseline, bump = 'patch') {
  versionParts(baseline);
  const index = { major: 0, minor: 1, patch: 2 }[bump];
  if (index === undefined) throw new Error('Bump must be patch, minor, or major.');
  const latest = stableTags(tags).at(-1);
  // The manifest version is the first release, or an intentional version bump.
  if (!latest || compare(baseline, latest) > 0) return baseline;
  const parts = versionParts(latest);
  parts[index]++;
  parts.fill(0, index + 1);
  const suggestion = `v${parts.join('.')}`;
  versionParts(suggestion);
  return suggestion;
}

export function validateNewVersion(tag, tags) {
  versionParts(tag);
  if (tags.includes(tag)) throw new Error(`${tag} already exists or is reserved by a draft release. Choose a new version.`);
  const latest = stableTags(tags).at(-1);
  if (latest && compare(tag, latest) <= 0) {
    throw new Error(`${tag} must be newer than ${latest}. Choose a new version.`);
  }
  return tag;
}

export function prVersion(labels) {
  const tags = labels.map(label => label.name).filter(name => name.startsWith('release:'));
  if (!tags.length) return null;
  if (tags.length !== 1) throw new Error('Use exactly one release:vMAJOR.MINOR.PATCH label on the PR.');
  const version = tags[0].slice('release:'.length);
  versionParts(version);
  return version;
}

export function approvedRevision(pr, reviews) {
  const latest = new Map();
  for (const review of reviews) {
    if (!review.user || !review.submitted_at || review.submitted_at > pr.merged_at ||
        !['APPROVED', 'CHANGES_REQUESTED', 'DISMISSED'].includes(review.state)) continue;
    latest.set(review.user.id, review);
  }
  const decisions = [...latest.values()];
  return decisions.some(r => r.state === 'APPROVED' && r.commit_id === pr.head.sha) &&
    !decisions.some(r => r.state === 'CHANGES_REQUESTED');
}

// A personal repository's owner can explicitly authorize a release by merging
// the PR. Use the recorded merger, never the actor rerunning the workflow.
export function mergedByOwner(pr, repo) {
  const owner = pr.base?.repo?.owner;
  return owner?.type === 'User' && Number.isSafeInteger(owner.id) &&
    owner.login?.toLowerCase() === repo.split('/')[0].toLowerCase() &&
    pr.merged_by?.id === owner.id;
}

async function optional(api, path) {
  try {
    return await api('GET', path);
  } catch (error) {
    if (error.status === 404) return null;
    throw error;
  }
}

export async function list(api, path) {
  const result = [];
  for (let page = 1; ; page++) {
    const items = await api('GET', `${path}${path.includes('?') ? '&' : '?'}per_page=100&page=${page}`);
    if (!Array.isArray(items)) throw new Error(`Expected a list from ${path}`);
    result.push(...items);
    if (items.length < 100) return result;
  }
}

export async function releaseTags(api, repo) {
  const tags = await list(api, `/repos/${repo}/tags`);
  const releases = await list(api, `/repos/${repo}/releases`);
  return [...new Set([...tags.map(t => t.name), ...releases.map(r => r.tag_name)])];
}

async function tagCommit(api, repo, tag) {
  const ref = await optional(api, `/repos/${repo}/git/ref/tags/${tag}`);
  if (!ref) return null;
  let object = ref.object;
  for (let depth = 0; object.type === 'tag' && depth < 10; depth++) {
    object = (await api('GET', `/repos/${repo}/git/tags/${object.sha}`)).object;
  }
  if (object.type !== 'commit') throw new Error(`Cannot resolve ${tag} to a commit.`);
  return object.sha;
}

const marker = sha => `<!-- wireless-pa-release:${sha} -->`;

async function existingRelease(api, repo, tag, sha) {
  versionParts(tag);
  // The by-tag endpoint promises published releases only; list also includes drafts.
  const release = (await list(api, `/repos/${repo}/releases`)).find(r => r.tag_name === tag);
  const taggedCommit = await tagCommit(api, repo, tag);
  if (release) {
    if (release.target_commitish !== sha || !release.body?.includes(marker(sha)) ||
        (taggedCommit && taggedCommit !== sha) || (!release.draft && !taggedCommit)) {
      throw new Error(`${tag} belongs to another release or commit; it will not be overwritten.`);
    }
    return release;
  }
  if (taggedCommit) throw new Error(`Git tag ${tag} already exists; choose a new version.`);
  return null;
}

export async function prepareRelease({ api, repo, eventName, event, sha, ref, actor, baseline }) {
  if (ref !== 'refs/heads/main') throw new Error('Releases and suggestions must run from main.');
  let tag;
  let prNumber = '';
  if (eventName === 'push') {
    const pulls = await list(api, `/repos/${repo}/commits/${sha}/pulls`);
    const matchesMerge = p => p.merged_at && p.base.ref === 'main' &&
      p.base.repo.full_name === repo && p.merge_commit_sha === sha;
    const match = pulls.find(matchesMerge);
    if (!match) return { run: false, reason: 'No merged PR for this commit; no build or release.' };
    // Commit association responses do not include merged_by. Fetch the full PR.
    const pr = await api('GET', `/repos/${repo}/pulls/${match.number}`);
    if (!matchesMerge(pr)) throw new Error('The PR no longer matches the release source commit.');
    tag = prVersion(pr.labels);
    if (!tag) return { run: false, reason: 'The merged PR has no release:vMAJOR.MINOR.PATCH label; no release.' };
    const reviews = await list(api, `/repos/${repo}/pulls/${pr.number}/reviews`);
    if (!mergedByOwner(pr, repo) && !approvedRevision(pr, reviews)) {
      throw new Error(`PR #${pr.number} requested ${tag}, but has neither an owner merge nor final-revision approval. Use the authorized manual release flow to recover.`);
    }
    prNumber = String(pr.number);
  } else if (eventName === 'workflow_dispatch') {
    const permission = await api('GET', `/repos/${repo}/collaborators/${encodeURIComponent(actor)}/permission`);
    if (!['write', 'maintain', 'admin'].includes(permission.permission)) {
      throw new Error('Manual release requires write, maintain, or admin access to this repository.');
    }
    const tags = await releaseTags(api, repo);
    if (event.inputs?.operation === 'suggest') {
      tag = suggestVersion(tags, baseline, event.inputs.bump || 'patch');
      return { run: false, tag, reason: `Suggested tag: ${tag}. Run again with operation=release and version=${tag}. No build was started.` };
    }
    if (event.inputs?.operation !== 'release') throw new Error('Choose the suggest or release operation.');
    tag = event.inputs.version?.trim();
    if (!tag) throw new Error('Provide the suggested version explicitly, for example v0.5.1.');
  } else {
    throw new Error(`Unsupported release event: ${eventName}`);
  }
  const existing = await existingRelease(api, repo, tag, sha);
  if (existing && !existing.draft) return { run: false, tag, reason: `${tag} is already published for this commit.` };
  if (!existing) validateNewVersion(tag, await releaseTags(api, repo));
  return { run: true, tag, sha, prNumber, reason: `Build and publish ${tag} from ${sha}.` };
}

export function stampVersion(tag, sha, root = '.') {
  versionParts(tag);
  if (!/^[a-f0-9]{40}$/.test(sha)) throw new Error('Expected a full source commit SHA.');
  const old = packageVersion(root).slice(1);
  const version = tag.slice(1);
  const manifestPath = join(root, 'Cargo.toml');
  const lockPath = join(root, 'Cargo.lock');
  const manifest = readFileSync(manifestPath, 'utf8');
  const lock = readFileSync(lockPath, 'utf8');
  const rootPackage = /(^\[\[package\]\]\r?\nname = "wireless-pa"\r?\nversion = ")[^"]+("\r?$)/m;
  if (!rootPackage.test(lock)) throw new Error('Cannot find wireless-pa in Cargo.lock.');
  writeFileSync(manifestPath, manifest.replace(`version = "${old}"`, `version = "${version}"`));
  writeFileSync(lockPath, lock.replace(rootPackage, `$1${version}$2`));
  writeFileSync(join(root, 'VERSION.txt'), `Wireless PA ${tag}\nSource commit: ${sha}\n`);
}

export async function publishRelease({ api, upload, repo, tag, sha, directory }) {
  versionParts(tag);
  const names = [
    `wireless-pa-${tag}-linux-x64.tar.gz`,
    `wireless-pa-${tag}-windows-x64.zip`,
    `wireless-pa-${tag}-windows-x64-setup.exe`,
  ];
  const checksums = names.map(name => {
    const file = join(directory, name);
    if (!statSync(file).isFile() || statSync(file).size === 0) throw new Error(`Missing or empty release asset: ${name}`);
    return `${createHash('sha256').update(readFileSync(file)).digest('hex')}  ${name}\n`;
  }).join('');
  writeFileSync(join(directory, 'SHA256SUMS'), checksums);
  let release = await existingRelease(api, repo, tag, sha);
  if (release && !release.draft) return release.html_url;
  if (!release) {
    validateNewVersion(tag, await releaseTags(api, repo));
    const notes = await api('POST', `/repos/${repo}/releases/generate-notes`, {
      tag_name: tag, target_commitish: sha,
    });
    release = await api('POST', `/repos/${repo}/releases`, {
      tag_name: tag, target_commitish: sha, name: `Wireless PA ${tag}`, draft: true,
      body: `${notes.body}\n\nSource commit: ${sha}\n\n${marker(sha)}`,
    });
  }
  // Upload only to our draft. If this fails, rerunning can resume it safely.
  await upload(tag, [...names, 'SHA256SUMS'].map(name => join(directory, name)));
  const actual = await list(api, `/repos/${repo}/releases/${release.id}/assets`);
  for (const name of [...names, 'SHA256SUMS']) {
    if (!actual.some(asset => asset.name === name && asset.state === 'uploaded' &&
        asset.size === statSync(join(directory, name)).size)) {
      throw new Error(`Release asset was not uploaded completely: ${name}`);
    }
  }
  // Pin the tag to the exact built commit before making the release visible.
  const taggedCommit = await tagCommit(api, repo, tag);
  if (taggedCommit && taggedCommit !== sha) throw new Error(`${tag} now points to another commit.`);
  if (!taggedCommit) await api('POST', `/repos/${repo}/git/refs`, { ref: `refs/tags/${tag}`, sha });
  const published = await api('PATCH', `/repos/${repo}/releases/${release.id}`, {
    draft: false, make_latest: 'true',
  });
  return published.html_url;
}
