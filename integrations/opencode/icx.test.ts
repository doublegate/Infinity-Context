// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
//
// `bun test integrations/opencode` with ICX_BIN pointing at a built icx (for example
// target/debug/icx). Drives the plugin through a fake V2 plugin context.

import { afterAll, beforeAll, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

type Handler = (event: any) => Promise<void> | void;

function fakeContext(events: any[] = []) {
  const hooks: Record<string, Handler> = {};
  const registration = (name: string, fn: Handler) => {
    hooks[name] = fn;
    return { dispose: async () => delete hooks[name] };
  };
  return {
    hooks,
    ctx: {
      session: { hook: async (name: string, fn: Handler) => registration(`session:${name}`, fn) },
      tool: { hook: async (name: string, fn: Handler) => registration(`tool:${name}`, fn) },
      event: {
        subscribe: async function* () {
          for (const e of events) yield e;
        },
      },
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

async function load(env: Record<string, string>, events: any[] = []) {
  Object.assign(process.env, env);
  const mod = await import(`./icx.ts?${Math.random()}`);
  const { hooks, ctx } = fakeContext(events);
  const dispose = await mod.default.setup(ctx);
  return { hooks, dispose };
}

test("prompt gets the legend and tail slot appended as user text", async () => {
  if (!process.env.ICX_BIN) return; // needs a built icx
  const { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "" });
  const event = { sessionID: "s1", prompt: { text: "why does the build fail" } };
  await hooks["session:prompt"](event);
  expect(event.prompt.text.startsWith("why does the build fail\n\n⟪ICX:LEGEND")).toBe(true);
  expect(event.prompt.text).toContain("⟪ICX:TAIL_SLOT│agent=open-code");
  await dispose();
});

test("execute.after replaces a large result only in stub mode", async () => {
  if (!process.env.ICX_BIN) return;
  let { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "" });
  const plain = { sessionID: "s1", tool: "bash", id: "c1", input: {}, status: "completed", result: { output: big } };
  await hooks["tool:execute.after"](plain);
  expect(plain.result.output).toBe(big);
  await dispose();

  ({ hooks, dispose } = await load({ ICX_OPENCODE_STUB: "1" }));
  const stubbed = { sessionID: "s1", tool: "bash", id: "c2", input: {}, status: "completed", result: { output: big, metadata: { m: 1 } } };
  await hooks["tool:execute.after"](stubbed);
  expect(stubbed.result.output.startsWith("⟪ICX:STUB│#icx:")).toBe(true);
  expect(stubbed.result.metadata).toEqual({ m: 1 });
  await dispose();
});

test("a stubbed MCP content list stays a list of content blocks", async () => {
  if (!process.env.ICX_BIN) return;
  const { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "1" });
  const event: any = {
    sessionID: "s1",
    tool: "mcp_github_search",
    id: "c3",
    input: {},
    status: "completed",
    result: { content: [{ type: "text", text: big.replace(/ok/g, "fine") }] },
  };
  await hooks["tool:execute.after"](event);
  expect(Array.isArray(event.result.content)).toBe(true);
  expect(event.result.content[0].type).toBe("text");
  expect(event.result.content[0].text.startsWith("⟪ICX:STUB│#icx:")).toBe(true);
  await dispose();
});

test("the legend is appended on a session's first prompt only", async () => {
  if (!process.env.ICX_BIN) return;
  const { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "" });
  const first = { sessionID: "s9", prompt: { text: "one" } };
  const second = { sessionID: "s9", prompt: { text: "two" } };
  const other = { sessionID: "s10", prompt: { text: "three" } };
  await hooks["session:prompt"](first);
  await hooks["session:prompt"](second);
  await hooks["session:prompt"](other);
  expect(first.prompt.text).toContain("⟪ICX:LEGEND");
  expect(second.prompt.text).not.toContain("⟪ICX:LEGEND");
  expect(second.prompt.text).toContain("⟪ICX:TAIL_SLOT");
  expect(other.prompt.text).toContain("⟪ICX:LEGEND");
  // A compaction may drop the prompt that carried the legend, so the next prompt sends it again.
  await hooks["session:compaction"]({ sessionID: "s9", messages: [] });
  const afterCompaction = { sessionID: "s9", prompt: { text: "four" } };
  await hooks["session:prompt"](afterCompaction);
  expect(afterCompaction.prompt.text).toContain("⟪ICX:LEGEND");
  await dispose();
});

test("a deleted session is sent as session.end, and unload still works", async () => {
  if (!process.env.ICX_BIN) return;
  // A wrapper that records each event name, then runs the real icx.
  const real = process.env.ICX_BIN;
  const log = join(root, "events.log");
  const wrapper = join(root, "icx-logging.sh");
  writeFileSync(wrapper, `#!/bin/sh\nprintf '%s\\n' "$5" >> '${log}'\nexec '${real}' "$@"\n`);
  chmodSync(wrapper, 0o755);
  try {
    const { hooks, dispose } = await load({ ICX_OPENCODE_STUB: "", ICX_BIN: wrapper }, [
      { type: "session.created", data: { sessionID: "s11" } },
      { type: "session.deleted", data: { sessionID: "s11" } },
    ]);
    expect(typeof hooks["session:prompt"]).toBe("function");
    // Wait for the event to be handled rather than relying on how many awaits setup() makes.
    const seen = () => existsSync(log) && readFileSync(log, "utf8").split("\n").includes("session.end");
    for (let i = 0; i < 100 && !seen(); i++) await new Promise((r) => setTimeout(r, 50));
    expect(seen()).toBe(true);
    await dispose();
    expect(hooks["session:prompt"]).toBeUndefined();
  } finally {
    process.env.ICX_BIN = real;
  }
});

test("a missing icx binary fails open", async () => {
  const saved = process.env.ICX_BIN;
  const { hooks, dispose } = await load({ ICX_BIN: join(root, "no-such-icx") });
  const prompt = { sessionID: "s1", prompt: { text: "hello" } };
  await hooks["session:prompt"](prompt);
  expect(prompt.prompt.text).toBe("hello");
  const tool = { sessionID: "s1", tool: "bash", status: "completed", result: { output: big } };
  await hooks["tool:execute.after"](tool);
  expect(tool.result.output).toBe(big);
  await hooks["session:compaction"]({ sessionID: "s1", messages: [] });
  await dispose();
  if (saved) process.env.ICX_BIN = saved;
});
