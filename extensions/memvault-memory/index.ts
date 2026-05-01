import { definePluginEntry } from "../../src/plugin-sdk/plugin-entry";

export default definePluginEntry({
  id: "memvault-memory",
  name: "Memvault Memory",
  description: "P2P collaborative memory store with knowledge graph and file attachments",
  kind: "memory",
  register(api) {
    const config = api.pluginConfig as {
      dataDir?: string;
      autoCapture?: boolean;
      autoRecall?: boolean;
      maxRecallResults?: number;
      defaultVisibility?: string;
      defaultTags?: string[];
    } | undefined;

    const autoCapture = config?.autoCapture ?? true;
    const autoRecall = config?.autoRecall ?? true;
    const maxResults = config?.maxRecallResults ?? 5;

    // Register as a memory corpus supplement (non-exclusive, coexists with other memory plugins)
    api.registerMemoryCorpusSupplement({
      async search(params) {
        // Search memvault via MCP tool call
        const results = await api.runtime.callTool("memvault_search", {
          query: params.query,
          limit: params.maxResults ?? maxResults,
        });

        if (!results || typeof results !== "string") return [];

        try {
          const parsed = JSON.parse(results);
          return (parsed.hits ?? []).map((hit: any) => ({
            corpus: "memvault",
            path: hit.cid,
            title: hit.title ?? undefined,
            kind: "memory",
            score: hit.score ?? 0.5,
            snippet: hit.snippet ?? "",
            source: "memvault",
            provenanceLabel: "memvault",
          }));
        } catch {
          return [];
        }
      },

      async get(params) {
        const result = await api.runtime.callTool("memvault_get", {
          cid: params.lookup,
        });

        if (!result || typeof result !== "string") return null;

        try {
          const parsed = JSON.parse(result);
          return {
            corpus: "memvault",
            path: params.lookup,
            content: parsed.body ?? parsed.text ?? result,
            title: parsed.title ?? undefined,
          };
        } catch {
          return null;
        }
      },
    });

    // Register memory prompt section (injects recall context)
    if (autoRecall) {
      api.registerMemoryPromptSupplement(({ availableTools }) => {
        if (!availableTools.has("memvault_search")) return [];
        return [
          "You have access to a persistent memory store (memvault). Use memvault_search to recall relevant information before answering questions that might benefit from prior context.",
          "When you learn important facts, preferences, or decisions, use memvault_put to store them for future recall.",
        ];
      });
    }

    // Register tools for direct agent access
    api.registerTool(
      {
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
      },
      { names: ["memvault_put"] }
    );

    api.registerTool(
      {
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
      },
      { names: ["memvault_search"] }
    );

    api.registerTool(
      {
        name: "memvault_get",
        description: "Retrieve a specific memory by its CID",
        parameters: {
          type: "object",
          properties: {
            cid: { type: "string", description: "Hex-encoded CID of the memory" },
          },
          required: ["cid"],
        },
      },
      { names: ["memvault_get"] }
    );

    // Auto-capture hook: after each conversation turn, extract and store key info
    if (autoCapture) {
      api.on("after_response", async (event: any) => {
        // The actual capture logic would analyze the response and extract
        // facts/decisions/preferences to store. This is a placeholder that
        // the MCP server handles via its own heuristics.
      });
    }
  },
});
