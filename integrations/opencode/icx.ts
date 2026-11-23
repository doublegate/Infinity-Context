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
//                  through the icx MCP tool. Verified on v2.2.7; opt-in, because it changes
//                  what the model sees.
//   compaction     seals icx's cache epoch; OpenCode still writes its own summary.
//   session.deleted (ctx.event.subscribe) ends the session's icx branches.
// The legend is appended on a session's first prompt, and again after each compaction, since
// each append stays in history and a compaction summary may drop it.
//
// Environment: ICX_BIN (path to the icx binary, default "icx"), ICX_PLUGIN_TIMEOUT_MS (default
// 10000; a hook on a store of several hundred MB can take seconds), and the usual icx
// variables (ICX_DIR, ...), which the icx process inherits.

import type { Plugin } from "@opencode/plugin";
import { spawnSync } from "node:child_process";

// Bounds on what is sent to icx and read back, in UTF-8 bytes; icx bounds its own input as well.
const MAX_SEND_BYTES = 64 * 1024 * 1024;
const MAX_REPLY_BYTES = 4 * 1024 * 1024;

type Reply = { append?: unknown; replace?: unknown };

/** Runs one icx hook. Any failure yields `{}`, so the caller changes nothing. */
function icxHook(event: string, payload: Record<string, unknown>): Reply {
  try {
    // Read per call, like the rest of the environment, so a change at run time takes effect.
    const bin = process.env.ICX_BIN || "icx";
    const timeout = Number(process.env.ICX_PLUGIN_TIMEOUT_MS) || 10000;
    const input = JSON.stringify(payload);
    if (Buffer.byteLength(input, "utf8") > MAX_SEND_BYTES) return {};
    const run = spawnSync(bin, ["hook", "--agent", "opencode", "--event", event], {
      input,
      // Bun passes only its startup environment to children unless `env` is given, so changes
      // made to process.env at run time (ICX_DIR, ...) would otherwise be lost.
      env: process.env,
      encoding: "utf8",
      timeout,
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
    // Sessions that already received the legend: each append stays in the conversation, so the
    // legend is sent once per session.
    const legendSent = new Set<string>();
    const prompt = await ctx.session.hook("prompt", async (event) => {
      const text = event?.prompt?.text;
      if (typeof text !== "string") return;
      const session = typeof event.sessionID === "string" ? event.sessionID : "";
      const reply = icxHook("prompt", {
        sessionID: event.sessionID,
        prompt: text,
        legend: !legendSent.has(session),
      });
      if (typeof reply.append === "string" && reply.append.length > 0) {
        event.prompt.text = `${text}\n\n${reply.append}`;
        legendSent.add(session);
      }
    });

    // A deleted session ends its icx branches. Event streams are best-effort: the plugin works
    // without them.
    const unload = new AbortController();
    const events = (async () => {
      if (typeof ctx.event?.subscribe !== "function") return;
      try {
        for await (const event of ctx.event.subscribe({ signal: unload.signal })) {
          // A stream that ignores the signal still stops here once unload starts.
          if (unload.signal.aborted) break;
          if (event?.type === "session.deleted") {
            const id = event.data?.sessionID;
            icxHook("session.end", { sessionID: id });
            legendSent.delete(typeof id === "string" ? id : "");
          }
        }
      } catch {
        // The stream ended or was aborted.
      }
    })();

    const after = await ctx.tool.hook("execute.after", async (event) => {
      const completed = event?.status === "completed";
      // A failed call can still carry partial output, which is stored with the error.
      const result = event?.result;
      // The result text is `content` or `output`, depending on the tool.
      const key = result && result.content !== undefined ? "content" : "output";
      const reply = icxHook("execute.after", {
        sessionID: event?.sessionID,
        tool: event?.tool,
        call_id: event?.id,
        input: event?.input,
        status: event?.status,
        output: result?.[key],
        error: event?.status === "error" ? event?.error?.message : undefined,
      });
      if (completed && typeof reply.replace === "string" && reply.replace.length > 0) {
        // An MCP `content` list stays a list of content blocks; a string stays a string.
        const stub = Array.isArray(result?.[key])
          ? [{ type: "text", text: reply.replace }]
          : reply.replace;
        event.result = { ...result, [key]: stub };
      }
    });

    const compaction = await ctx.session.hook("compaction", async (event) => {
      // Never set event.result: OpenCode writes its own summary.
      icxHook("compaction", { sessionID: event?.sessionID });
      // The summary may drop the prompt that carried the legend, so the next prompt sends it
      // again.
      legendSent.delete(typeof event?.sessionID === "string" ? event.sessionID : "");
    });

    return async () => {
      unload.abort();
      // Wait at most a second for the event loop, and clear the timer either way, so unload
      // never keeps the process alive.
      let timer: ReturnType<typeof setTimeout> | undefined;
      await Promise.race([events, new Promise((resolve) => (timer = setTimeout(resolve, 1000)))]);
      clearTimeout(timer);
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
