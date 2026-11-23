// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
//
// `bun test integrations/opencode` with ICX_BIN pointing at a built icx (for example
// target/debug/icx). Drives the plugin through a fake V2 plugin context.

import { afterAll, beforeAll, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

type Handler = (event: any) => Promise<void> | void;

function fakeContext() {
  const hooks: Record<string, Handler> = {};
  const registration = (name: string, fn: Handler) => {
    hooks[name] = fn;
    return { dispose: async () => delete hooks[name] };
  };
  return {
    hooks,
    ctx: {
      session: { hook: async (name: string, fn: Handler) => registration(`session.${name}`, fn) },
      tool: { hook: async (name: string, fn: Handler) => registration(`tool.${name}`, fn) },
    },
  };
}

let root = "";
const big = Array.from({ length: 400 }, (_, i) => `line ${i}: case_${i} ... ok`).join("\n");

beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), "icx-opencode-plugin-"));
  Object.assign(process.env, {
    ICX_DIR: join(root, "ws"),
    ICX_STATE_HOME: join(root, "state"),
    ICX_CONFIG_HOME: join(root, "config"),
    XDG_RUNTIME_DIR: join(root, "run"),
    ICX_NO_DAEMON: "1",
  });
  // `icx hook` runs the adapter only for a ready workspace.
  if (process.env.ICX_BIN) {
    spawnSync(process.env.ICX_BIN, ["init", "--dir", join(root, "ws")], {
      stdio: "ignore",
      env: process.env,
    });
  }
});

afterAll(() => rmSync(root, { recursive: true, force: true }));

async function load(env: Record<string, string>) {
  Object.assign(process.env, env);
  // A fresh module instance, so ICX_BIN is read again.
  const mod = await import(`./icx.ts?${Math.random()}`);
  const { hooks, ctx } = fakeContext();
  const dispose = await mod.default.setup(ctx);
  return { hooks, dispose };
}

test("prompt gets the legend and tail slot appended as user text", async () => {
  if (!process.env.ICX_BIN) return; // needs a built icx
  const { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "" });
  const event = { sessionID: "s1", prompt: { text: "why does the build fail" } };
  await hooks["session.prompt"](event);
  expect(event.prompt.text.startsWith("why does the build fail\n\n⟪ICX:LEGEND")).toBe(true);
  expect(event.prompt.text).toContain("⟪ICX:TAIL_SLOT│agent=open-code");
  await dispose();
});

test("execute.after replaces a large result only in stub mode", async () => {
  if (!process.env.ICX_BIN) return;
  let { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "" });
  const plain = { sessionID: "s1", tool: "bash", id: "c1", input: {}, status: "completed", result: { output: big } };
  await hooks["tool.execute.after"](plain);
  expect(plain.result.output).toBe(big);
  await dispose();

  ({ hooks, dispose } = await load({ ICX_OPENCODE_STUB: "1" }));
  const stubbed = { sessionID: "s1", tool: "bash", id: "c2", input: {}, status: "completed", result: { output: big, metadata: { m: 1 } } };
  await hooks["tool.execute.after"](stubbed);
  expect(stubbed.result.output.startsWith("⟪ICX:STUB│#icx:")).toBe(true);
  expect(stubbed.result.metadata).toEqual({ m: 1 });
  await dispose();
});

test("a missing icx binary fails open", async () => {
  const saved = process.env.ICX_BIN;
  const { hooks, dispose } = await load({ ICX_BIN: join(root, "no-such-icx") });
  const prompt = { sessionID: "s1", prompt: { text: "hello" } };
  await hooks["session.prompt"](prompt);
  expect(prompt.prompt.text).toBe("hello");
  const tool = { sessionID: "s1", tool: "bash", status: "completed", result: { output: big } };
  await hooks["tool.execute.after"](tool);
  expect(tool.result.output).toBe(big);
  await hooks["session.compaction"]({ sessionID: "s1", messages: [] });
  await dispose();
  if (saved) process.env.ICX_BIN = saved;
});
