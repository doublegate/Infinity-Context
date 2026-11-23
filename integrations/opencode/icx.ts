// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later
//
// Infinity-Context (icx) plugin for OpenCode (V2 plugin API; written against OpenCode v2.2.7).
//
// Install: copy this file to ~/.config/opencode/plugins/icx.ts (global) or to a project's
// .opencode/plugins/icx.ts. It calls `icx hook --agent opencode --event <EVENT>` and applies the
// reply. It fails open: if icx is missing, errors, times out or prints anything unexpected,
// OpenCode runs unchanged.
//
// Hooks used:
//   prompt         appends the icx legend and tail slot to the user's message (user-turn data;
//                  icx never writes system text).
//   execute.after  ingests each tool result. With ICX_OPENCODE_STUB=1 in icx's environment, a
//                  large completed result is replaced by a stub whose #icx: handle dereferences
//                  through the icx MCP tool (opt-in until verified on your OpenCode version).
//   compaction     seals icx's cache epoch; OpenCode still writes its own summary.
//
// Environment: ICX_BIN (path to the icx binary, default "icx"), ICX_PLUGIN_TIMEOUT_MS (default
// 5000), and the usual icx variables (ICX_DIR, ...), which the icx process inherits.

import type { Plugin } from "@opencode/plugin";
import { spawnSync } from "node:child_process";

const ICX_BIN = process.env.ICX_BIN || "icx";
const TIMEOUT_MS = Number(process.env.ICX_PLUGIN_TIMEOUT_MS) || 5000;
// Bounds on what is sent to icx and read back; icx bounds its own input as well.
const MAX_SEND_BYTES = 64 * 1024 * 1024;
const MAX_REPLY_BYTES = 4 * 1024 * 1024;

type Reply = { append?: unknown; replace?: unknown };

/** Runs one icx hook. Any failure yields `{}`, so the caller changes nothing. */
function icxHook(event: string, payload: Record<string, unknown>): Reply {
  try {
    const input = JSON.stringify(payload);
    if (input.length > MAX_SEND_BYTES) return {};
    const run = spawnSync(ICX_BIN, ["hook", "--agent", "opencode", "--event", event], {
      input,
      // Bun passes only its startup environment to children unless `env` is given, so changes
      // made to process.env at run time (ICX_DIR, ...) would otherwise be lost.
      env: process.env,
      encoding: "utf8",
      timeout: TIMEOUT_MS,
      maxBuffer: MAX_REPLY_BYTES,
      stdio: ["pipe", "pipe", "ignore"],
    });
    if (run.error || run.status !== 0 || typeof run.stdout !== "string") return {};
    const parsed: unknown = JSON.parse(run.stdout.trim() || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Reply) : {};
  } catch {
    return {};
  }
}

const IcxPlugin: Plugin.Plugin = {
  id: "icx",
  setup: async (ctx) => {
    const prompt = await ctx.session.hook("prompt", async (event) => {
      const text = event?.prompt?.text;
      if (typeof text !== "string") return;
      const reply = icxHook("prompt", { sessionID: event.sessionID, prompt: text });
      if (typeof reply.append === "string" && reply.append.length > 0) {
        event.prompt.text = `${text}\n\n${reply.append}`;
      }
    });

    const after = await ctx.tool.hook("execute.after", async (event) => {
      const completed = event?.status === "completed";
      const result = completed ? event.result : undefined;
      // The result text is `content` or `output`, depending on the tool.
      const key = result && result.content !== undefined ? "content" : "output";
      const reply = icxHook("execute.after", {
        sessionID: event?.sessionID,
        tool: event?.tool,
        call_id: event?.id,
        input: event?.input,
        status: event?.status,
        output: completed ? result?.[key] : undefined,
        error: event?.status === "error" ? event?.error?.message : undefined,
      });
      if (completed && typeof reply.replace === "string" && reply.replace.length > 0) {
        event.result = { ...result, [key]: reply.replace };
      }
    });

    const compaction = await ctx.session.hook("compaction", async (event) => {
      // Never set event.result: OpenCode writes its own summary.
      icxHook("compaction", { sessionID: event?.sessionID });
    });

    return async () => {
      for (const registration of [prompt, after, compaction]) {
        try {
          await registration.dispose();
        } catch {
          // Unload is best-effort.
        }
      }
    };
  },
};

export default IcxPlugin;
