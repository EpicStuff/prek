import std/[json, streams, strformat, strutils]

type
  Diagnostic = object
    message: string
    severity: string
    ruleId: string
    filePath: string
    startLine: int
    startColumn: int
    endLine: int
    endColumn: int

proc expectField(node: JsonNode; key: string): JsonNode =
  if not node.hasKey(key):
    raise newException(ValueError, fmt"Missing required field '{key}'")
  node[key]

proc getLineCol(rangeNode: JsonNode; key: string): (int, int) =
  let pointNode = expectField(rangeNode, key)
  let line = expectField(pointNode, "line").getInt() + 1
  let character = expectField(pointNode, "character").getInt() + 1
  (line, character)

proc parseDiagnostic(node: JsonNode): Diagnostic =
  let message = expectField(node, "message").getStr()
  let severity = node{"severity"}.getStr("warning")

  let ruleId =
    if node.hasKey("rule"):
      node["rule"].getStr()
    elif node.hasKey("code"):
      node["code"].getStr()
    else:
      "basedpyright"

  let filePath =
    if node.hasKey("file"):
      node["file"].getStr()
    elif node.hasKey("path"):
      node["path"].getStr()
    else:
      raise newException(ValueError, "Missing required field 'file' or 'path'")

  let rangeNode = expectField(node, "range")
  let (startLine, startColumn) = getLineCol(rangeNode, "start")
  let (endLine, endColumn) = getLineCol(rangeNode, "end")

  Diagnostic(
    message: message,
    severity: severity,
    ruleId: if ruleId.len == 0: "basedpyright" else: ruleId,
    filePath: filePath,
    startLine: startLine,
    startColumn: startColumn,
    endLine: endLine,
    endColumn: endColumn
  )

proc parseDiagnostics(root: JsonNode): seq[Diagnostic] =
  let diagsNode =
    if root.kind == JArray:
      root
    elif root.kind == JObject and root.hasKey("generalDiagnostics"):
      root["generalDiagnostics"]
    elif root.kind == JObject and root.hasKey("diagnostics"):
      root["diagnostics"]
    else:
      raise newException(ValueError, "Unable to locate diagnostics array (expected array, 'generalDiagnostics', or 'diagnostics')")

  if diagsNode.kind != JArray:
    raise newException(ValueError, "Diagnostics collection must be an array")

  for idx, item in diagsNode.elems.pairs:
    try:
      result.add(parseDiagnostic(item))
    except ValueError as exc:
      raise newException(ValueError, fmt"Invalid diagnostic at index {idx}: {exc.msg}")

proc mapLevel(severity: string): string =
  case severity.toLowerAscii()
  of "error":
    "error"
  of "warning":
    "warning"
  of "information", "info", "hint":
    "note"
  else:
    "warning"

proc toSarif(diagnostics: seq[Diagnostic]): JsonNode =
  var results = newJArray()

  for diag in diagnostics:
    results.add(%*{
      "ruleId": diag.ruleId,
      "level": mapLevel(diag.severity),
      "message": {"text": diag.message},
      "locations": [
        {
          "physicalLocation": {
            "artifactLocation": {"uri": diag.filePath},
            "region": {
              "startLine": diag.startLine,
              "startColumn": diag.startColumn,
              "endLine": diag.endLine,
              "endColumn": diag.endColumn
            }
          }
        }
      ]
    })

  %*{
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "version": "2.1.0",
    "runs": [
      {
        "tool": {"driver": {"name": "basedpyright"}},
        "results": results
      }
    ]
  }

proc readAllStdin(): string =
  let input = newFileStream(stdin)
  if input.isNil:
    raise newException(IOError, "Unable to read stdin")
  input.readAll()

proc main() =
  try:
    let rawInput = readAllStdin()
    if rawInput.len == 0:
      raise newException(ValueError, "No input received on stdin")

    let parsed = parseJson(rawInput)
    let diagnostics = parseDiagnostics(parsed)
    let sarif = toSarif(diagnostics)

    stdout.write($sarif)
    stdout.write("\n")
  except CatchableError as exc:
    stderr.writeLine("bp2sarif error: " & exc.msg)
    quit(QuitFailure)

when isMainModule:
  main()
