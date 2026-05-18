<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **quantum-mechanics-music** (1053 symbols, 1933 relationships, 91 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/quantum-mechanics-music/context` | Codebase overview, check index freshness |
| `gitnexus://repo/quantum-mechanics-music/clusters` | All functional areas |
| `gitnexus://repo/quantum-mechanics-music/processes` | All execution flows |
| `gitnexus://repo/quantum-mechanics-music/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |
| Work in the Config area (44 symbols) | `.claude/skills/generated/config/SKILL.md` |
| Work in the Tui area (25 symbols) | `.claude/skills/generated/tui/SKILL.md` |
| Work in the Runtime area (21 symbols) | `.claude/skills/generated/runtime/SKILL.md` |
| Work in the Cluster_11 area (8 symbols) | `.claude/skills/generated/cluster-11/SKILL.md` |
| Work in the Cluster_27 area (7 symbols) | `.claude/skills/generated/cluster-27/SKILL.md` |
| Work in the Cluster_43 area (7 symbols) | `.claude/skills/generated/cluster-43/SKILL.md` |
| Work in the Cluster_44 area (7 symbols) | `.claude/skills/generated/cluster-44/SKILL.md` |
| Work in the Cluster_36 area (6 symbols) | `.claude/skills/generated/cluster-36/SKILL.md` |

<!-- gitnexus:end -->
