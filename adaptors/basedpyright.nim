import std/[json, strutils]

proc getIntOr(node: JsonNode; key: string; defaultValue: int): int =
  if node.kind == JObject and node.hasKey(key) and node[key].kind == JInt:
    node[key].getInt
  else:
    defaultValue

proc getStrOr(node: JsonNode; key: string; defaultValue: string): string =
  if node.kind == JObject and node.hasKey(key) and node[key].kind == JString:
    node[key].getStr
  else:
    defaultValue

proc mapLevel(severity: string): string =
  case severity.toLowerAscii()
  of "error":
    "error"
  of "warning":
    "warning"
  of "information", "info":
    "note"
  else:
    "warning"

proc diagnosticToResult(diag: JsonNode): JsonNode =
  let severity = getStrOr(diag, "severity", "warning")
  let message = getStrOr(diag, "message", "")
  let filePath = getStrOr(diag, "file", "")
  let ruleId = getStrOr(diag, "rule", "")

  let rangeNode = if diag.kind == JObject and diag.hasKey("range"): diag["range"] else: newJNull()
  let startNode = if rangeNode.kind == JObject and rangeNode.hasKey("start"): rangeNode["start"] else: newJObject()
  let endNode = if rangeNode.kind == JObject and rangeNode.hasKey("end"): rangeNode["end"] else: newJObject()

  let startLine = getIntOr(startNode, "line", 0) + 1
  let startColumn = getIntOr(startNode, "character", 0) + 1
  let endLine = getIntOr(endNode, "line", startLine - 1) + 1
  let endColumn = getIntOr(endNode, "character", startColumn - 1) + 1

  var result = %*{
    "level": mapLevel(severity),
    "message": {"text": message},
    "locations": [
      {
        "physicalLocation": {
          "artifactLocation": {"uri": filePath},
          "region": {
            "startLine": startLine,
            "startColumn": startColumn,
            "endLine": endLine,
            "endColumn": endColumn
          }
        }
      }
    ]
  }

  if ruleId.len > 0:
    result["ruleId"] = %ruleId

  result

proc convertBasedPyright(input: JsonNode): JsonNode =
  let version = getStrOr(input, "version", "")
  var results = newJArray()

  if input.kind == JObject and input.hasKey("generalDiagnostics") and input["generalDiagnostics"].kind == JArray:
    for diag in input["generalDiagnostics"].items:
      results.add(diagnosticToResult(diag))

  var driver = %*{"name": "basedpyright"}
  if version.len > 0:
    driver["semanticVersion"] = %version

  %*{
    "version": "2.1.0",
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "runs": [
      {
        "tool": {
          "driver": driver
        },
        "results": results
      }
    ]
  }

proc main() =
  let rawInput = stdin.readAll()
  if rawInput.strip().len == 0:
    stdout.write("{\"version\":\"2.1.0\",\"$schema\":\"https://json.schemastore.org/sarif-2.1.0.json\",\"runs\":[]}")
    return

  let parsed = parseJson(rawInput)

  if parsed.kind == JObject and parsed.hasKey("runs"):
    stdout.write(rawInput)
    return

  let sarif = convertBasedPyright(parsed)
  stdout.write($sarif)

main()
