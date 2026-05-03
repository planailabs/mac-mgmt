import { definePluginEntry } from "openclaw/plugin-sdk/plugin-entry";
import { readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

function textResult(text: string) {
  return { content: [{ type: "text" as const, text }], details: undefined };
}

function errorResult(msg: string) {
  return { content: [{ type: "text" as const, text: `Error: ${msg}` }], details: { error: true } };
}

function loadToken(): string {
  const dataDir = join(homedir(), ".local/share/memvault");
  return readFileSync(join(dataDir, "api.token"), "utf-8").trim();
}

async function memvaultFetch(
  apiUrl: string,
  path: string,
  opts: { method?: string; body?: unknown } = {},
): Promise<unknown> {
  const token = loadToken();
  const res = await fetch(`${apiUrl}${path}`, {
    method: opts.method ?? "GET",
    headers: {
      Authorization: `Bearer ${token}`,
      ...(opts.body ? { "Content-Type": "application/json" } : {}),
    },
    body: opts.body ? JSON.stringify(opts.body) : undefined,
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
      autoCapture?: boolean;
      autoRecall?: boolean;
      maxRecallResults?: number;
      defaultVisibility?: string;
      defaultTags?: string[];
    } | undefined;

    const apiUrl = config?.apiUrl ?? "http://127.0.0.1:8401";
    const autoRecall = config?.autoRecall ?? true;
    const maxResults = config?.maxRecallResults ?? 5;
    const defaultTags = config?.defaultTags ?? ["agent:openclaw"];
    const defaultVisibility = config?.defaultVisibility ?? "internal";

    // Register as a memory corpus supplement
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

    // Inject recall prompt
    if (autoRecall) {
      api.registerMemoryPromptSupplement(() => [
        "You have access to a persistent memory store (memvault). Use memvault_search to recall relevant information before answering questions that might benefit from prior context.",
        "When you learn important facts, preferences, or decisions, use memvault_put to store them for future recall.",
      ]);
    }

    // memvault_put — store a memory
    api.registerTool({
      name: "memvault_put",
      description: "Store a memory in the persistent p2p memory store",
      parameters: {
        type: "object",
        properties: {
          text: { type: "string", description: "Memory content (markdown)" },
          title: { type: "string", description: "Optional title" },
          tags: { type: "array", items: { type: "string" }, description: "Tags in scope:label format" },
        },
        required: ["text"],
      },
      async execute(_id: string, params: { text: string; title?: string; tags?: string[] }) {
        try {
          const tags: Array<[string, string]> = [];
          for (const t of params.tags ?? defaultTags) {
            const [ns, ...rest] = t.split(":");
            tags.push([ns, rest.join(":") || ns]);
          }
          const frontmatter: Record<string, unknown> = {};
          if (params.title) frontmatter.title = params.title;

          const doc = (await memvaultFetch(apiUrl, "/api/v1/docs", {
            method: "POST",
            body: {
              body: params.text,
              frontmatter: Object.keys(frontmatter).length > 0 ? frontmatter : undefined,
              tags,
              visibility: defaultVisibility,
            },
          })) as { id: string; cid: string };

          return textResult(JSON.stringify({ stored: true, id: doc.id, cid: doc.cid }));
        } catch (err: unknown) {
          return errorResult(err instanceof Error ? err.message : String(err));
        }
      },
    });

    // memvault_search — search memories
    api.registerTool({
      name: "memvault_search",
      description: "Search the persistent memory store",
      parameters: {
        type: "object",
        properties: {
          query: { type: "string", description: "Search query" },
          limit: { type: "number", description: "Max results (default: 5)" },
        },
        required: ["query"],
      },
      async execute(_id: string, params: { query: string; limit?: number }) {
        try {
          const limit = params.limit ?? maxResults;
          const hits = await memvaultFetch(
            apiUrl,
            `/api/v1/search?q=${encodeURIComponent(params.query)}&limit=${limit}`,
          );
          return textResult(JSON.stringify(hits));
        } catch (err: unknown) {
          return errorResult(err instanceof Error ? err.message : String(err));
        }
      },
    });

    // memvault_get — retrieve a specific memory
    api.registerTool({
      name: "memvault_get",
      description: "Retrieve a specific memory by its document ID",
      parameters: {
        type: "object",
        properties: {
          id: { type: "string", description: "Hex-encoded document ID" },
        },
        required: ["id"],
      },
      async execute(_id: string, params: { id: string }) {
        try {
          const doc = await memvaultFetch(apiUrl, `/api/v1/docs/${params.id}`);
          return textResult(JSON.stringify(doc));
        } catch (err: unknown) {
          return errorResult(err instanceof Error ? err.message : String(err));
        }
      },
    });
  },
});
