# Output hashes for git dependencies in Cargo.lock.
# Used by all package.nix files via cargoLock.outputHashes.
# All swiftide-* crates come from the same git repo; each needs its own entry.
let hash = "sha256-zN6IQ+4YzXYTLCKOqSJBnj/V0aIr7031HO+xDBQlgck=";
in {
  "swiftide-0.32.1" = hash;
  "swiftide-agents-0.32.1" = hash;
  "swiftide-core-0.32.1" = hash;
  "swiftide-indexing-0.32.1" = hash;
  "swiftide-integrations-0.32.1" = hash;
  "swiftide-macros-0.32.1" = hash;
  "swiftide-query-0.32.1" = hash;
}
