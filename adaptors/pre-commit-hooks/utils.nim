import std/json

proc buildSarif*(toolName: string; results: JsonNode): JsonNode =
  %*{
    "version": "2.1.0",
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "runs": [{"tool": {"driver": {"name": toolName}}, "results": results}]
  }

proc writeSarif*(toolName: string; results: JsonNode) =
  stdout.write($(buildSarif(toolName, results)))
  stdout.write("\n")
