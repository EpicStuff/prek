import std/[json, re, strutils]

let diagRe = re"^(.+?):([0-9]+):(.*)$"

proc sanitizeRuleToken(token: string): string =
  var cleaned = newStringOfCap(token.len)
  for ch in token:
    if ch.isAlphaNumeric or ch in {'_', '-'}:
      cleaned.add(ch.toLowerAscii)
    elif cleaned.len == 0 or cleaned[^1] != '-':
      cleaned.add('-')

  result = cleaned.strip(chars = {'-'})

proc deriveRuleId(details: string): string =
  let parts = details.split("==>", maxsplit = 1)
  if parts.len == 2:
    let misspelling = parts[0].strip()
    let token = sanitizeRuleToken(misspelling)
    if token.len > 0:
      return "codespell/" & token

  "codespell"

proc toSarifResult(filePath: string, lineNum: int, details: string): JsonNode =
  result = %*{
    "ruleId": deriveRuleId(details),
    "level": "warning",
    "message": {
      "text": details
    },
    "locations": [
      {
        "physicalLocation": {
          "artifactLocation": {
            "uri": filePath
          },
          "region": {
            "startLine": lineNum
          }
        }
      }
    ]
  }

proc main() =
  let input = stdin.readAll()
  var findings = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip(trailing = false)
    if line.strip().len == 0:
      continue

    var matches: array[3, string]
    if line.match(diagRe, matches):
      try:
        let filePath = matches[0]
        let lineNum = parseInt(matches[1])
        let details = matches[2].strip()
        findings.add(toSarifResult(filePath, lineNum, details))
      except ValueError:
        stderr.writeLine("warning: malformed codespell line (invalid line number): " & line)
      continue

    if line.contains(":") and line.contains("==>"):
      stderr.writeLine("warning: malformed codespell line: " & line)

  let sarif = %*{
    "version": "2.1.0",
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "runs": [
      {
        "tool": {
          "driver": {
            "name": "codespell"
          }
        },
        "results": findings
      }
    ]
  }

  stdout.write($sarif)
  stdout.write("\n")

main()
