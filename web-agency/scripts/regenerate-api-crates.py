#!/usr/bin/env nix-shell
#!nix-shell -i python3 -p python3 python3Packages.requests yq-go
"""
Download, trim, and regenerate the API client crates using cargo-progenitor.

Usage:
    ./web-agency/scripts/regenerate-api-crates.py

Requires:
    - cargo-progenitor: cargo install cargo-progenitor
"""

import json
import os
import re
import subprocess
import sys
import shutil
import tempfile

import requests

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
WEB_AGENCY_DIR = os.path.dirname(SCRIPT_DIR)
ROOT_DIR = os.path.dirname(WEB_AGENCY_DIR)

CF_SPEC_URL = "https://raw.githubusercontent.com/cloudflare/api-schemas/main/openapi.yaml"
CF_YAML = os.path.join(WEB_AGENCY_DIR, "openapi-cloudflare.yaml")
CF_TRIMMED = os.path.join(WEB_AGENCY_DIR, "cloudflare-api", "openapi-trimmed.json")
CF_OUTPUT = os.path.join(WEB_AGENCY_DIR, "cloudflare-api")

SS_DOCS_URL = "https://docs.spaceship.dev"
SS_JSON = os.path.join(WEB_AGENCY_DIR, "openapi-spaceship.json")
SS_TRIMMED = os.path.join(WEB_AGENCY_DIR, "spaceship-api", "openapi-trimmed.json")
SS_OUTPUT = os.path.join(WEB_AGENCY_DIR, "spaceship-api")

CD_DOCS_URL = "https://changedetection.io/docs/api_v1/index.html"
CD_JSON = os.path.join(WEB_AGENCY_DIR, "openapi-changedetection.json")
CD_TRIMMED = os.path.join(WEB_AGENCY_DIR, "changedetection-api", "openapi-trimmed.json")
CD_OUTPUT = os.path.join(WEB_AGENCY_DIR, "changedetection-api")

# Paths we need from the Cloudflare API
CF_KEEP_PATHS = [
    "/accounts",
    "/accounts/{account_id}",
    "/zones",
    "/zones/{zone_id}",
    "/zones/{zone_id}/dns_records",
    "/zones/{zone_id}/dns_records/{dns_record_id}",
    "/zones/{zone_id}/dnssec",
    "/zones/{zone_id}/settings/ssl_automatic_mode",
    "/zones/{zone_id}/bot_management",
    "/accounts/{account_id}/pages/projects",
    "/accounts/{account_id}/pages/projects/{project_name}",
    "/accounts/{account_id}/pages/projects/{project_name}/domains",
    "/accounts/{account_id}/pages/projects/{project_name}/domains/{domain_name}",
    "/accounts/{account_id}/registrar/domain-check",
    "/accounts/{account_id}/registrar/registrations",
    "/accounts/{account_id}/registrar/registrations/{domain_name}",
]


def download_cloudflare_spec():
    """Download the Cloudflare OpenAPI spec from GitHub."""
    print(f"Downloading Cloudflare spec from {CF_SPEC_URL}...")
    resp = requests.get(CF_SPEC_URL, timeout=120)
    resp.raise_for_status()
    with open(CF_YAML, "wb") as f:
        f.write(resp.content)
    print(f"  Written to {CF_YAML} ({len(resp.content)} bytes)")


def fix_double_encoded_utf8(obj):
    """Fix double-encoded UTF-8 in JSON strings.

    The Redoc page stores the spec with mojibake: UTF-8 bytes of characters
    like smart quotes are stored as individual latin-1 codepoints in JSON
    unicode escapes (e.g. \\u00e2\\u0080\\u009c instead of \\u201c).
    This re-encodes each string as latin-1 and decodes as UTF-8 to fix them.
    """
    if isinstance(obj, str):
        try:
            return obj.encode("latin-1").decode("utf-8")
        except (UnicodeDecodeError, UnicodeEncodeError):
            return obj
    elif isinstance(obj, dict):
        return {k: fix_double_encoded_utf8(v) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [fix_double_encoded_utf8(v) for v in obj]
    return obj


def download_redoc_spec(docs_url, output_path, name):
    """Download an OpenAPI spec by extracting __redoc_state from a Redoc docs page."""
    print(f"Downloading {name} spec from {docs_url}...")
    resp = requests.get(docs_url, timeout=60)
    resp.raise_for_status()
    m = re.search(r'__redoc_state\s*=\s*(.*?)\s*;\s*\n', resp.text, re.DOTALL)
    if not m:
        print(f"Could not find __redoc_state in {name} docs page", file=sys.stderr)
        sys.exit(1)
    state = json.loads(m.group(1))
    spec = fix_double_encoded_utf8(state["spec"]["data"])
    with open(output_path, "w") as f:
        json.dump(spec, f, indent=2, ensure_ascii=False)
    print(f"  Written to {output_path} (paths: {len(spec.get('paths', {}))})")


def download_spaceship_spec():
    """Download the Spaceship OpenAPI spec by extracting it from their Redoc docs page."""
    download_redoc_spec(SS_DOCS_URL, SS_JSON, "Spaceship")


def download_changedetection_spec():
    """Download the ChangeDetection.io OpenAPI spec by extracting it from their Redoc docs page."""
    download_redoc_spec(CD_DOCS_URL, CD_JSON, "ChangeDetection.io")


def yaml_to_json(yaml_path):
    """Convert YAML to JSON using yq."""
    result = subprocess.run(
        ["yq", "-o=json", yaml_path],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"yq failed: {result.stderr}", file=sys.stderr)
        sys.exit(1)
    return json.loads(result.stdout)


def collect_refs(obj, refs=None):
    """Recursively collect all $ref schema references."""
    if refs is None:
        refs = set()
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k == "$ref" and isinstance(v, str) and v.startswith("#/components/schemas/"):
                refs.add(v.split("/")[-1])
            if k == "$ref" and isinstance(v, str) and v.startswith("#/components/parameters/"):
                refs.add(("param", v.split("/")[-1]))
            collect_refs(v, refs)
    elif isinstance(obj, list):
        for item in obj:
            collect_refs(item, refs)
    return refs


def fix_enum_bools(obj):
    """Fix enum values: bools in boolean-typed enums stay bool, bools in string-typed enums become strings."""
    if isinstance(obj, dict):
        t = obj.get("type")
        if "enum" in obj and isinstance(obj["enum"], list):
            new_enum = []
            for v in obj["enum"]:
                if isinstance(v, bool):
                    if t == "string":
                        new_enum.append(str(v).lower())
                    else:
                        new_enum.append(v)
                else:
                    new_enum.append(v)
            obj["enum"] = new_enum
        for v in obj.values():
            if isinstance(v, (dict, list)):
                fix_enum_bools(v)
    elif isinstance(obj, list):
        for item in obj:
            if isinstance(item, (dict, list)):
                fix_enum_bools(item)


def add_missing_request_body_schemas(spec):
    """Add empty object schemas to request bodies that lack them."""
    for path, methods in spec.get("paths", {}).items():
        for method, op in methods.items():
            if isinstance(op, dict) and "requestBody" in op:
                for ct, ct_val in op["requestBody"].get("content", {}).items():
                    if "schema" not in ct_val:
                        ct_val["schema"] = {"type": "object"}


def simplify_dns_schemas(schemas):
    """Simplify complex DNS record schemas that progenitor can't handle."""
    # TTL: replace anyOf with simple number
    if "dns-records_ttl" in schemas:
        schemas["dns-records_ttl"] = {
            "type": "number",
            "description": "TTL in seconds. 1 means automatic.",
            "example": 3600,
            "default": 1,
        }

    # DNS record union types: simplify to permissive objects
    record_union_schemas = [
        "dns-records_dns-record-post", "dns-records_dns-record-patch",
        "dns-records_dns-record-response", "dns-records_dns-record-shared-fields",
        "dns-records_dns-record-with-data", "dns-records_dns-record-without-data",
    ]
    for name in record_union_schemas:
        if name in schemas:
            schemas[name] = {
                "type": "object",
                "description": f"DNS record (simplified). Original: {name}",
                "properties": {
                    "id": {"type": "string"},
                    "type": {"type": "string"},
                    "name": {"type": "string"},
                    "content": {"type": "string"},
                    "ttl": {"type": "number"},
                    "proxied": {"type": "boolean"},
                    "proxiable": {"type": "boolean"},
                    "comment": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "data": {"type": "object", "additionalProperties": True},
                    "priority": {"type": "number"},
                    "created_on": {"type": "string"},
                    "modified_on": {"type": "string"},
                },
                "additionalProperties": True,
            }

    # Individual record type schemas: simplify to generic objects
    for name in list(schemas.keys()):
        if name.startswith("dns-records_") and name.endswith("Record"):
            schemas[name] = {
                "type": "object",
                "additionalProperties": True,
            }


def simplify_anyof(obj):
    """Convert anyOf to oneOf where possible, simplify same-type anyOf."""
    if isinstance(obj, dict):
        if "anyOf" in obj and len(obj["anyOf"]) > 0:
            types = set()
            for item in obj["anyOf"]:
                types.add(item.get("type", "ref"))
            if len(types) == 1 and "ref" not in types:
                t = types.pop()
                merged = {"type": t}
                for item in obj["anyOf"]:
                    merged.update({k: v for k, v in item.items() if k != "type"})
                del obj["anyOf"]
                obj.update(merged)
            elif len(obj["anyOf"]) > 1:
                obj["oneOf"] = obj.pop("anyOf")
        for v in obj.values():
            if isinstance(v, (dict, list)):
                simplify_anyof(v)
    elif isinstance(obj, list):
        for item in obj:
            if isinstance(item, (dict, list)):
                simplify_anyof(item)


def trim_cloudflare():
    """Trim the Cloudflare OpenAPI spec."""
    print("Converting Cloudflare YAML to JSON...")

    # Try yq first, fall back to reading pre-converted JSON
    json_path = os.path.join(tempfile.gettempdir(), "cf-openapi.json")
    try:
        spec = yaml_to_json(CF_YAML)
        with open(json_path, "w") as f:
            json.dump(spec, f)
    except FileNotFoundError:
        print("yq not found, trying pre-converted JSON...")
        if os.path.exists(json_path):
            with open(json_path) as f:
                spec = json.load(f)
        else:
            print("No pre-converted JSON found. Install yq: nix-shell -p yq-go", file=sys.stderr)
            sys.exit(1)

    full_schemas = spec.get("components", {}).get("schemas", {})
    full_params = spec.get("components", {}).get("parameters", {})

    # Filter paths
    filtered_paths = {p: v for p, v in spec.get("paths", {}).items() if p in CF_KEEP_PATHS}
    print(f"  Paths: {len(filtered_paths)}")

    # Collect referenced schemas (iteratively)
    all_schemas = {}
    all_params = {}

    refs = collect_refs(filtered_paths)
    schema_refs = {r for r in refs if isinstance(r, str)}
    param_refs = {r[1] for r in refs if isinstance(r, tuple)}

    # Add parameters
    for p in param_refs:
        if p in full_params:
            all_params[p] = full_params[p]

    seen = set()
    while schema_refs - seen:
        new = schema_refs - seen
        for name in new:
            if name in full_schemas:
                all_schemas[name] = full_schemas[name]
                collect_refs(full_schemas[name], schema_refs)
        seen |= new

    # Also collect schema refs from parameters
    param_schema_refs = collect_refs(all_params)
    param_schema_refs = {r for r in param_schema_refs if isinstance(r, str)}
    for name in param_schema_refs:
        if name in full_schemas and name not in all_schemas:
            all_schemas[name] = full_schemas[name]

    print(f"  Schemas: {len(all_schemas)}")
    print(f"  Parameters: {len(all_params)}")

    # Build trimmed spec
    trimmed = {
        "openapi": spec.get("openapi", "3.0.3"),
        "info": {"title": "Cloudflare API (trimmed for web-agency)", "version": "4.0.0"},
        "servers": spec.get("servers", []),
        "paths": filtered_paths,
        "components": {
            "schemas": all_schemas,
            "parameters": all_params,
            "securitySchemes": spec.get("components", {}).get("securitySchemes", {}),
        },
    }

    # Apply fixes
    print("  Fixing enum bools...")
    fix_enum_bools(trimmed)
    print("  Fixing missing request body schemas...")
    add_missing_request_body_schemas(trimmed)
    print("  Simplifying DNS schemas...")
    simplify_dns_schemas(trimmed["components"]["schemas"])
    print("  Simplifying anyOf patterns...")
    simplify_anyof(trimmed)

    os.makedirs(os.path.dirname(CF_TRIMMED), exist_ok=True)
    with open(CF_TRIMMED, "w") as f:
        json.dump(trimmed, f, indent=2)
    print(f"  Written to {CF_TRIMMED}")
    return trimmed


def generate_crate(spec_path, output_dir, crate_name, extra_deps=None, post_gen_fixups=None):
    """Generate an API crate using cargo-progenitor, preserving hand-maintained files.

    Args:
        spec_path: path to the (trimmed) OpenAPI JSON spec
        output_dir: target crate directory
        crate_name: Cargo package name
        extra_deps: dict of {cargo_line: package_name} for deps needed by compat.rs
        post_gen_fixups: callable(code) -> code applied to generated lib.rs before swap
    """
    progenitor = os.path.expanduser("~/.cargo/bin/cargo-progenitor")
    if not os.path.exists(progenitor):
        print("cargo-progenitor not found. Install: cargo install cargo-progenitor", file=sys.stderr)
        sys.exit(1)

    tmp_output = output_dir + "-gen"
    if os.path.exists(tmp_output):
        shutil.rmtree(tmp_output)

    print(f"Generating {crate_name} crate...")
    result = subprocess.run(
        [progenitor, "progenitor",
         "-i", spec_path,
         "-o", tmp_output,
         "-n", crate_name,
         "-v", "0.1.0"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"Generation failed: {result.stderr}", file=sys.stderr)
        sys.exit(1)
    print(f"  Generated to {tmp_output}")

    # Apply post-generation fixups to lib.rs
    if post_gen_fixups:
        lib_rs = os.path.join(tmp_output, "src", "lib.rs")
        with open(lib_rs) as f:
            code = f.read()
        code = post_gen_fixups(code)
        with open(lib_rs, "w") as f:
            f.write(code)

    # Swap old crate with new, preserving hand-maintained files
    old_backup = output_dir + "-old"
    if os.path.exists(old_backup):
        shutil.rmtree(old_backup)

    preserved = {}
    if os.path.exists(output_dir):
        for fname in ["openapi-trimmed.json", "src/compat.rs"]:
            fpath = os.path.join(output_dir, fname)
            if os.path.exists(fpath):
                with open(fpath, "rb") as f:
                    preserved[fname] = f.read()
        tests_dir = os.path.join(output_dir, "tests")
        if os.path.exists(tests_dir):
            preserved["tests"] = tests_dir
        shutil.move(output_dir, old_backup)

    shutil.move(tmp_output, output_dir)

    # Restore preserved files
    for fname, data in preserved.items():
        if fname == "tests":
            shutil.copytree(data, os.path.join(output_dir, "tests"))
        else:
            fpath = os.path.join(output_dir, fname)
            os.makedirs(os.path.dirname(fpath), exist_ok=True)
            with open(fpath, "wb") as f:
                f.write(data)

    # Inject `pub mod compat;` into lib.rs if compat.rs was preserved
    compat_path = os.path.join(output_dir, "src", "compat.rs")
    if os.path.exists(compat_path):
        lib_rs = os.path.join(output_dir, "src", "lib.rs")
        with open(lib_rs) as f:
            code = f.read()
        if "pub mod compat;" not in code:
            code = code.replace(
                "pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};",
                "pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};\n\npub mod compat;",
            )
            with open(lib_rs, "w") as f:
                f.write(code)

    # Merge extra deps needed by compat.rs into Cargo.toml
    if extra_deps:
        cargo_toml = os.path.join(output_dir, "Cargo.toml")
        with open(cargo_toml) as f:
            cargo = f.read()
        for line, pkg in extra_deps.items():
            if pkg not in cargo:
                cargo = cargo.rstrip() + "\n" + line + "\n"
        with open(cargo_toml, "w") as f:
            f.write(cargo)

    # Clean up
    if os.path.exists(old_backup):
        shutil.rmtree(old_backup)

    print(f"  Crate ready at {output_dir}")


def trim_spaceship():
    """Trim the Spaceship OpenAPI spec (light touch — spec is small)."""
    print("Loading Spaceship OpenAPI JSON...")
    with open(SS_JSON) as f:
        spec = json.load(f)

    print(f"  Paths: {len(spec.get('paths', {}))}")
    print(f"  Schemas: {len(spec.get('components', {}).get('schemas', {}))}")

    # Apply generic fixes
    print("  Fixing enum bools...")
    fix_enum_bools(spec)
    print("  Simplifying anyOf patterns...")
    simplify_anyof(spec)

    os.makedirs(os.path.dirname(SS_TRIMMED), exist_ok=True)
    with open(SS_TRIMMED, "w") as f:
        json.dump(spec, f, indent=2)
    print(f"  Written to {SS_TRIMMED}")
    return spec


def cf_post_gen_fixups(code):
    """Post-generation fixups for the Cloudflare crate."""
    code = code.replace(
        '#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]\n    pub struct DnsRecordsForAZoneDeleteDnsRecordResponseResultInner',
        '#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, PartialEq)]\n    pub struct DnsRecordsForAZoneDeleteDnsRecordResponseResultInner',
    )
    return code


CF_EXTRA_DEPS = {
    'thiserror = "2"': "thiserror",
    'tracing = "0.1"': "tracing",
}

SS_EXTRA_DEPS = {
    'thiserror = "2"': "thiserror",
    'tracing = "0.1"': "tracing",
    'tokio = { version = "1", features = ["time"] }': "tokio",
}


def downgrade_openapi_31_types(obj):
    """Convert OpenAPI 3.1 type arrays to 3.0 nullable format.

    3.1 uses `type: ["string", "null"]` for nullable fields; progenitor
    expects 3.0 style `type: "string", nullable: true`.
    """
    if isinstance(obj, dict):
        if "type" in obj and isinstance(obj["type"], list):
            types = [t for t in obj["type"] if t != "null"]
            nullable = "null" in obj["type"]
            if len(types) == 1:
                obj["type"] = types[0]
            elif len(types) > 1:
                # Multiple non-null types: pick the first (best effort)
                obj["type"] = types[0]
            else:
                # Only null — use string as fallback
                obj["type"] = "string"
            if nullable:
                obj["nullable"] = True
        for v in obj.values():
            if isinstance(v, (dict, list)):
                downgrade_openapi_31_types(v)
    elif isinstance(obj, list):
        for item in obj:
            if isinstance(item, (dict, list)):
                downgrade_openapi_31_types(item)


def trim_changedetection():
    """Trim the ChangeDetection.io OpenAPI spec (light touch — spec is small)."""
    print("Loading ChangeDetection.io OpenAPI JSON...")
    with open(CD_JSON) as f:
        spec = json.load(f)

    print(f"  Paths: {len(spec.get('paths', {}))}")
    print(f"  Schemas: {len(spec.get('components', {}).get('schemas', {}))}")

    # Downgrade 3.1 → 3.0 for progenitor compatibility
    print("  Downgrading OpenAPI 3.1 type arrays to 3.0 nullable format...")
    spec["openapi"] = "3.0.3"
    downgrade_openapi_31_types(spec)

    # Apply generic fixes
    print("  Fixing enum bools...")
    fix_enum_bools(spec)
    print("  Simplifying anyOf patterns...")
    simplify_anyof(spec)

    os.makedirs(os.path.dirname(CD_TRIMMED), exist_ok=True)
    with open(CD_TRIMMED, "w") as f:
        json.dump(spec, f, indent=2)
    print(f"  Written to {CD_TRIMMED}")
    return spec


if __name__ == "__main__":
    download_cloudflare_spec()
    download_spaceship_spec()
    download_changedetection_spec()

    trim_cloudflare()
    generate_crate(CF_TRIMMED, CF_OUTPUT, "cloudflare-api",
                   extra_deps=CF_EXTRA_DEPS, post_gen_fixups=cf_post_gen_fixups)

    trim_spaceship()
    generate_crate(SS_TRIMMED, SS_OUTPUT, "spaceship-api",
                   extra_deps=SS_EXTRA_DEPS)

    trim_changedetection()
    generate_crate(CD_TRIMMED, CD_OUTPUT, "changedetection-api")

    print("\nDone! Run 'cargo check -p cloudflare-api -p spaceship-api -p changedetection-api' to verify.")
