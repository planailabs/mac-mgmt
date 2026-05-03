import { definePluginEntry } from "openclaw/plugin-sdk/plugin-entry";
import { readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

function loadToken(): string {
  const dataDir = join(homedir(), ".local/share/memvault");
  return readFileSync(join(dataDir, "api.token"), "utf-8").trim();
}

async function memvaultFetch(apiUrl: string, path: string): Promise<unknown> {
  const token = loadToken();
  const res = await fetch(`${apiUrl}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`${res.status} ${res.statusText}: ${text}`);
  }
  return res.json();
}

export default definePluginEntry({
  id: "memvault-memory",
  name: "Memvault Memory",
  description: "P2P collaborative memory store with knowledge graph and file attachments",
  kind: "memory",
  register(api) {
    const config = api.pluginConfig as {
      apiUrl?: string;
      autoRecall?: boolean;
      maxRecallResults?: number;
    } | undefined;

    const apiUrl = config?.apiUrl ?? "http://127.0.0.1:8401";
    const autoRecall = config?.autoRecall ?? true;
    const maxResults = config?.maxRecallResults ?? 5;

    // Hook into OpenClaw's memory system via corpus supplement.
    // The actual tools (memvault_put, memvault_search, etc.) are provided
    // by the plan-ai-memvault MCP server registered in mcp.servers.
    api.registerMemoryCorpusSupplement({
      async search(params) {
        try {
          const limit = params.maxResults ?? maxResults;
          const hits = (await memvaultFetch(
            apiUrl,
            `/api/v1/search?q=${encodeURIComponent(params.query)}&limit=${limit}`,
          )) as Array<{ doc_id: string; score: number; snippet: string }>;

          return hits.map((hit) => ({
            corpus: "memvault",
            path: hit.doc_id,
            title: undefined,
            kind: "memory",
            score: hit.score,
            snippet: hit.snippet,
            source: "memvault",
            provenanceLabel: "memvault",
          }));
        } catch {
          return [];
        }
      },

      async get(params) {
        try {
          const doc = (await memvaultFetch(apiUrl, `/api/v1/docs/${params.lookup}`)) as {
            id: string;
            body: string;
            frontmatter: Record<string, unknown>;
          };
          return {
            corpus: "memvault",
            path: params.lookup,
            content: doc.body,
            title: (doc.frontmatter?.title as string) ?? undefined,
          };
        } catch {
          return null;
        }
      },
    });

    if (autoRecall) {
      api.registerMemoryPromptSupplement(() => [
        "You have access to a persistent memory store (memvault). Use memvault_search to recall relevant information before answering questions that might benefit from prior context.",
        "When you learn important facts, preferences, or decisions, use memvault_put to store them for future recall.",
      ]);
    }
  },
});
