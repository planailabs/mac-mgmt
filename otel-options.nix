# OpenTelemetry options shared by the server, daemon and relay modules.
#
# Each service reads `[opentelemetry]` from its own config file, so the
# endpoint and any non-secret headers are rendered there. Secrets go through
# `otlpHeadersFile` instead: the process gives an OTEL_* variable already in
# the environment precedence over its config, so the file overrides whatever
# the Nix store holds.
{ lib }:

{
  otlpEndpoint = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    example = "http://localhost:4318";
    description = ''
      OTLP/HTTP collector base URL for traces, metrics and logs. Unset means
      nothing is exported; metrics are still collected and scrapeable from
      the service's own /metrics endpoint either way.

      Other OTEL_* variables (sampling, resource attributes) can be passed
      through {option}`environmentFile`.
    '';
  };

  otlpHeaders = lib.mkOption {
    type = lib.types.attrsOf lib.types.str;
    default = { };
    example = {
      "X-Scope-OrgID" = "mac-mgmt";
    };
    description = ''
      Headers sent with every OTLP export.

      ::: {.warning}
      These values land in the world-readable Nix store. Put API keys and
      other credentials in {option}`otlpHeadersFile` instead.
      :::
    '';
  };

  otlpHeadersFile = lib.mkOption {
    type = lib.types.nullOr lib.types.path;
    default = null;
    example = "/run/secrets/otlp-headers.env";
    description = ''
      File holding the OTLP export headers, kept out of the Nix store.
      A single line of `OTEL_EXPORTER_OTLP_HEADERS=key=value,key2=value2`
      (comma-separated, as the OpenTelemetry spec defines it) — typically
      the collector's auth header. Overrides {option}`otlpHeaders`.
    '';
  };
}
