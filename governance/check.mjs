#!/usr/bin/env node
import { lstat, readFile, readdir } from 'node:fs/promises';
import { dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..');
const fail = (message) => { console.error(`[governance] ${message}`); process.exitCode = 1; };
const sha40 = /^[0-9a-f]{40}$/;
const sha64 = /^[0-9a-f]{64}$/;
const expectedValidatorRevision = 'bd503465dab8c5148fee722b443ed04ff126c9bf';

function within(base, target) {
  const r = relative(base, target);
  return r === '' || (!r.startsWith(`..${sep}`) && r !== '..' && !isAbsolute(r));
}
function repoPath(value, label) {
  if (typeof value !== 'string' || !value || value.includes('\0') || isAbsolute(value)) throw new Error(`${label} must be repository-relative`);
  const absolute = resolve(root, value);
  if (!within(root, absolute)) throw new Error(`${label} escapes repository root`);
  return absolute;
}
async function realFile(value, label) {
  const absolute = repoPath(value, label);
  const st = await lstat(absolute);
  if (st.isSymbolicLink() || !st.isFile()) throw new Error(`${label} must be a real file: ${value}`);
  return absolute;
}
async function realDirectory(value, label) {
  const absolute = repoPath(value, label);
  const st = await lstat(absolute);
  if (st.isSymbolicLink() || !st.isDirectory()) throw new Error(`${label} must be a real directory: ${value}`);
  return absolute;
}

try {
  const policy = JSON.parse(await readFile(await realFile('governance/api-server.v1.json', 'API governance manifest'), 'utf8'));
  if (policy.schema !== 'ores.governance.api-server/v1' || policy.repository !== 'ores-otel/ores-otel-api-server.rs') throw new Error('invalid API governance identity');
  if (policy.routeRoot !== 'src/routes' || policy.legacyFlatRoutesAllowed !== false) throw new Error('unsafe route-root policy');
  if (!Array.isArray(policy.requiredIngressNamespaces) || policy.requiredIngressNamespaces.length !== 4) throw new Error('expected four ingress namespaces');
  const required = [...policy.requiredIngressNamespaces].sort();
  if (JSON.stringify(required) !== JSON.stringify(['graphql','rest','rpc','ws'])) throw new Error('canonical ingress namespaces must be rest/rpc/graphql/ws');

  const routeRoot = await realDirectory(policy.routeRoot, 'route root');
  const observedDirs = [];
  const looseFiles = [];
  for (const entry of (await readdir(routeRoot, { withFileTypes: true })).sort((a,b) => a.name.localeCompare(b.name))) {
    if (entry.isSymbolicLink()) throw new Error(`symlink forbidden in route root: ${entry.name}`);
    if (entry.isDirectory()) observedDirs.push(entry.name);
    else if (entry.isFile()) looseFiles.push(entry.name);
    else throw new Error(`unsupported route-root entry: ${entry.name}`);
  }
  if (JSON.stringify(observedDirs.sort()) !== JSON.stringify(required)) throw new Error(`route namespace drift; expected=${required.join(',')} actual=${observedDirs.sort().join(',')}`);
  if (looseFiles.length !== 1 || looseFiles[0] !== 'mod.rs') throw new Error(`legacy flat routes forbidden; route root files=${looseFiles.join(',')}`);
  for (const namespace of required) await realFile(`${policy.routeRoot}/${namespace}/mod.rs`, `${namespace} ingress module`);

  if (!Array.isArray(policy.restLeaves) || policy.restLeaves.length === 0) throw new Error('restLeaves must be non-empty');
  const restDir = await realDirectory(`${policy.routeRoot}/rest`, 'REST route root');
  const restFiles = (await readdir(restDir, { withFileTypes: true })).filter((e) => e.isFile()).map((e) => e.name);
  for (const leaf of policy.restLeaves) {
    if (!/^[A-Za-z0-9_-]+\.rs$/.test(leaf) || !restFiles.includes(leaf)) throw new Error(`missing/invalid governed REST leaf: ${leaf}`);
  }

  const domain = JSON.parse(await readFile(await realFile(policy.domainAuthorityBinding, 'domain authority binding'), 'utf8'));
  if (domain.schema !== 'ores.contract-authority-binding/v1' || domain.repository !== 'ores-otel/ores-otel-interfaces') throw new Error('invalid OTEL domain authority');
  if (!sha40.test(domain.commit) || domain.path !== 'contracts/' || domain.mode !== 'external-pinned') throw new Error('domain authority must be immutable and externally pinned');
  for (const key of ['contractsSha256','corpusSha256','conformanceSpecSha256']) if (!sha64.test(domain.receipts?.[key])) throw new Error(`missing/invalid domain authority receipt: ${key}`);

  const admission = JSON.parse(await readFile(await realFile(policy.contractAdmissionManifest, 'contract admission manifest'), 'utf8'));
  if (admission.schema !== 'ores.contract-ir-consumer/v1' || admission.repository !== policy.repository || admission.repositoryRole !== 'api-server') throw new Error('invalid contract admission identity');
  if (admission.canonicalAuthorityRepository !== 'ores-otel/ores-interfaces') throw new Error('local admission canary authority drift');
  if (admission.canonicalAuthorityRepository === domain.repository) throw new Error('domain authority and generic admission canary must remain distinct scopes');
  if (admission.validator?.repository !== 'ORESoftware/typespec-json-schema-validator' || admission.validator?.actionCommit !== expectedValidatorRevision) throw new Error('validator provenance drift');
  if (admission.authorityModel?.precedence !== 'none' || admission.authorityModel?.generatedJsonSchema !== 'comparison-evidence-only' || admission.admission?.allowFallbackAuthority !== false) throw new Error('unsafe peer-authority policy');
  for (const key of ['requirePassedReceipt','requireAdmissibleContractIr','requireZeroUnexplainedFindings','rejectEditableAuthority']) if (admission.admission?.[key] !== true) throw new Error(`contract admission must fail closed: ${key}`);

  await realFile(policy.generatedReadme, 'generated-tree authority marker');
  const conformance = JSON.parse(await readFile(await realFile('conformance/manifest.v1.json', 'conformance manifest'), 'utf8'));
  if (conformance.repository !== policy.repository || conformance.coverage?.status !== 'scaffold-only') throw new Error('API conformance boundary must begin scaffold-only');
  if (conformance.policy?.runtimeSpecificGoldensAllowed !== false || conformance.policy?.generatedEvidenceIsAuthority !== false) throw new Error('unsafe conformance policy');

  console.log(JSON.stringify({
    schema: 'ores.governance.api-server-check/v1',
    repository: policy.repository,
    ingress_namespaces: required,
    rest_leaves: [...policy.restLeaves].sort(),
    domain_authority: { repository: domain.repository, commit: domain.commit },
    admission_canary_authority: admission.canonicalAuthorityRepository,
    validator_revision: admission.validator.actionCommit,
    generated_readme: policy.generatedReadme
  }, null, 2));
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
