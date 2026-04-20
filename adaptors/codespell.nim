import std/[json, parseutils, strutils]

type
  Finding = object
    uri: string
    lineNo: int
    message: string

proc parseDiagnosticLine(line: string): Finding =
  ## Parse codespell output lines with pattern: <path>:<line>: <details>
  ## Returns empty finding (lineNo = 0) for non-diagnostic / malformed lines.
  var markerStart = -1
  var markerEnd = -1

  var i = 0
  while i < line.len:
    if line[i] == ':' and i + 2 < line.len and line[i + 1].isDigitAscii():
      var j = i + 1
      while j < line.len and line[j].isDigitAscii():
        inc j
      if j < line.len and line[j] == ':':
        markerStart = i
        markerEnd = j
        break
    inc i

  if markerStart <= 0 or markerEnd <= markerStart + 1:
    return

  let uri = line[0 ..< markerStart].strip()
  let lineNoText = line[markerStart + 1 ..< markerEnd].strip()
  let message = line[markerEnd + 1 .. ^1].strip()

  if uri.len == 0 or lineNoText.len == 0 or message.len == 0:
    return

  var lineNo = 0
  if parseInt(lineNoText, lineNo) == 0 or lineNo <= 0:
    return

  result = Finding(uri: uri, lineNo: lineNo, message: message)

proc findingToResult(finding: Finding): JsonNode =
  result = %*{
    "ruleId": "codespell",
    "level": "warning",
    "message": {
      "text": finding.message
    },
    "locations": [
      {
        "physicalLocation": {
          "artifactLocation": {
            "uri": finding.uri
          },
          "region": {
            "startLine": finding.lineNo
          }
        }
      }
    ]
  }

let input = stdin.readAll()
var results: seq[JsonNode] = @[]

for rawLine in input.splitLines():
  let line = rawLine.strip()
  if line.len == 0:
    continue
  let finding = parseDiagnosticLine(line)
  if finding.lineNo == 0:
    continue
  results.add(findingToResult(finding))

let report = %*{
  "version": "2.1.0",
  "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "codespell"
        }
      },
      "results": results
    }
  ]
}

stdout.write($report)
