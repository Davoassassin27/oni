#!/usr/bin/env node
/**
 * ONI end-to-end smoke test.
 *
 * Requires a running daemon. Start it first:
 *   cargo run --manifest-path oni-daemon/Cargo.toml
 *
 * Then:
 *   npm run smoke                 # fake agent, no model tokens
 *   npm run smoke:opencode        # real OpenCode, uses model tokens
 *
 * The script creates a disposable git repo, registers it in Oni, dispatches a
 * task, waits for review, checks the patch, merges and verifies the result.
 */

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const args = parseArgs(process.argv.slice(2));
const baseUrl = (args.url ?? "http://127.0.0.1:7766").replace(/\/$/, "");
const agent = args.agent === "opencode" ? "opencode" : "custom";
const timeoutMs =
  Number(args.timeout ?? (agent === "opencode" ? 300 : 120)) * 1000;
const keep = Boolean(args.keep);

const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const repoName = `oni-smoke-${stamp}`;
const repoDir = mkdtempSync(join(tmpdir(), "oni-smoke-repo-"));
let projectId = null;
let taskId = null;
let succeeded = false;

try {
  await main();
  succeeded = true;
} catch (error) {
  console.error(`\nFAIL: ${error instanceof Error ? error.message : error}`);
  if (taskId) {
    try {
      const log = await apiText(`/tasks/${taskId}/log`);
      console.error("\n--- task log (tail) ---");
      console.error(log.split("\n").slice(-50).join("\n"));
    } catch {
      // The daemon may be down.
    }
  }
  if (!keep) {
    console.error(`\nArtifacts kept for debugging: ${repoDir}`);
  }
  process.exitCode = 1;
} finally {
  if (projectId) {
    try {
      await api(`/projects/${projectId}`, { method: "DELETE" });
    } catch {
      // Best effort.
    }
  }
  if (succeeded && !keep) {
    rmSync(repoDir, { recursive: true, force: true });
  } else if (succeeded) {
    console.log(`Kept repo: ${repoDir}`);
  }
}

async function main() {
  console.log(`ONI smoke test · agent=${agent} · ${baseUrl}`);
  const health = await api("/health");
  console.log(`daemon v${health.version}, ${health.tasks} tasks, ${health.projects} projects`);

  initRepo();
  console.log(`repo: ${repoDir}`);

  const project = await api("/projects", {
    method: "POST",
    body: {
      name: repoName,
      path: repoDir,
      baseBranch: "main",
      verifyCommand: "node verify.js",
      agent,
      customCommand: agent === "custom" ? "node agent.js" : null,
    },
  });
  projectId = project.id;
  console.log(`project: ${project.id}`);

  const prompt =
    agent === "opencode"
      ? "Create a file named smoke.txt in the repository root with exactly the text oni-smoke and a trailing newline. Do not modify any other file."
      : "Create smoke.txt with the text oni-smoke. Read the prompt from stdin, then finish.";

  const task = await api("/tasks", {
    method: "POST",
    body: {
      projectId,
      title: `Smoke test ${stamp}`,
      prompt,
      acceptance: "node verify.js",
      agent,
      customCommand: agent === "custom" ? "node agent.js" : null,
      maxIterations: 2,
    },
  });
  taskId = task.id;
  console.log(`task: ${taskId} (${task.state})`);

  const finished = await waitForTask(taskId, timeoutMs);
  console.log(`state: ${finished.state} · attempt ${finished.iteration}/${finished.maxIterations}`);

  if (finished.state !== "review") {
    throw new Error(
      `expected review, got ${finished.state}${finished.lastError ? `: ${finished.lastError.slice(0, 300)}` : ""}`,
    );
  }

  const patch = await api(`/tasks/${taskId}/patch`);
  const file = patch.files?.find((item) => item.path === "smoke.txt");
  if (!file) {
    throw new Error(
      `patch does not contain smoke.txt (files: ${(patch.files ?? []).map((item) => item.path).join(", ") || "none"})`,
    );
  }
  console.log(`patch: ${patch.files.length} files, +${file.insertions} -${file.deletions}`);
  if (!file.patch.includes("+oni-smoke")) {
    throw new Error("patch does not contain the expected +oni-smoke line");
  }

  const merged = await api(`/tasks/${taskId}/merge`, { method: "POST" });
  if (merged.state !== "merged") {
    throw new Error(`expected merged, got ${merged.state}`);
  }
  console.log("merge: ok");

  const mergedFile = join(repoDir, "smoke.txt");
  if (!existsSync(mergedFile)) {
    throw new Error("smoke.txt is missing from the base branch after merge");
  }
  if (!readFileSync(mergedFile, "utf8").includes("oni-smoke")) {
    throw new Error("smoke.txt has unexpected content after merge");
  }
  const status = git(["status", "--porcelain"]);
  if (status.trim()) {
    throw new Error(`base repo is dirty after merge:\n${status}`);
  }
  console.log("base branch: smoke.txt present, repo clean");
  console.log("\nPASS");
}

function initRepo() {
  git(["init", "-b", "main"]);
  git(["config", "user.name", "Oni Smoke"]);
  git(["config", "user.email", "smoke@oni.local"]);
  writeFileSync(join(repoDir, "README.md"), "# smoke\n");
  writeFileSync(
    join(repoDir, "verify.js"),
    [
      'const fs = require("node:fs");',
      'if (!fs.existsSync("smoke.txt")) { console.error("missing smoke.txt"); process.exit(1); }',
      'const text = fs.readFileSync("smoke.txt", "utf8").trim();',
      'if (text !== "oni-smoke") { console.error("unexpected smoke.txt:", text); process.exit(1); }',
      'console.log("verify: smoke.txt is valid");',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(repoDir, "agent.js"),
    [
      'const fs = require("node:fs");',
      "let prompt = '';",
      'process.stdin.setEncoding("utf8");',
      'process.stdin.on("data", (chunk) => { prompt += chunk; });',
      'process.stdin.on("end", () => {',
      '  fs.writeFileSync("smoke.txt", "oni-smoke\\n");',
      '  console.log("agent: wrote smoke.txt");',
      '  console.log(`agent: prompt bytes = ${prompt.length}`);',
      "});",
      "",
    ].join("\n"),
  );
  git(["add", "."]);
  git(["-c", "user.name=Oni Smoke", "-c", "user.email=smoke@oni.local", "commit", "-m", "smoke baseline"]);
}

function git(gitArgs) {
  return execFileSync("git", gitArgs, { cwd: repoDir, encoding: "utf8" });
}

async function waitForTask(id, maxMs) {
  const started = Date.now();
  let last = "";
  while (Date.now() - started < maxMs) {
    const task = await api(`/tasks/${id}`);
    if (task.state !== last) {
      console.log(`  ${task.state}${task.iteration ? ` · attempt ${task.iteration}/${task.maxIterations}` : ""}`);
      last = task.state;
    }
    if (["review", "failed", "stopped", "merged", "discarded"].includes(task.state)) {
      return task;
    }
    await sleep(1000);
  }
  throw new Error(`timed out after ${maxMs / 1000}s waiting for task ${id}`);
}

async function api(path, options = {}) {
  const response = await fetch(`${baseUrl}/api${path}`, {
    method: options.method ?? "GET",
    headers: options.body ? { "Content-Type": "application/json" } : undefined,
    body: options.body ? JSON.stringify(options.body) : undefined,
    signal: AbortSignal.timeout(20_000),
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${options.method ?? "GET"} ${path} → ${response.status}: ${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function apiText(path) {
  const response = await fetch(`${baseUrl}/api${path}`, {
    signal: AbortSignal.timeout(20_000),
  });
  const text = await response.text();
  if (!response.ok) throw new Error(`${path} → ${response.status}`);
  return text;
}

function sleep(ms) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, ms));
}

function parseArgs(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 1) {
    const item = argv[index];
    if (!item.startsWith("--")) continue;
    const key = item.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      result[key] = true;
    } else {
      result[key] = next;
      index += 1;
    }
  }
  return result;
}
