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

################################

import std/json
import std/strutils

proc normalizePath*(path: string): string =
  path.replace('\\', '/')

proc messageResult*(ruleId: string, message: string): JsonNode =
  %*{
    "ruleId": ruleId,
    "level": "error",
    "message": {
      "text": message
    }
  }

proc locationResult*(ruleId: string, message: string, path: string): JsonNode =
  %*{
    "ruleId": ruleId,
    "level": "error",
    "message": {
      "text": message
    },
    "locations": [
      {
        "physicalLocation": {
          "artifactLocation": {
            "uri": path.normalizePath()
          }
        }
      }
    ]
  }

proc buildSarif*(ruleId: string, toolName: string, shortDescription: string, results: JsonNode): JsonNode =
  %*{
    "version": "2.1.0",
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "runs": [
      {
        "tool": {
          "driver": {
            "name": toolName,
            "informationUri": "https://github.com/pre-commit/pre-commit-hooks",
            "rules": [
              {
                "id": ruleId,
                "name": toolName,
                "shortDescription": {
                  "text": shortDescription
                }
              }
            ]
          }
        },
        "results": results
      }
    ]
  }
