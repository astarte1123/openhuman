# Minutes Skill Packs

Skill packs are small JSON bundles that group existing Minutes plugin skills
into a named workflow package. The point is not to invent a second plugin
system. The point is to make the existing skill graph installable, recommendable,
and checkable by agents without guessing.

## File format

Each pack is a JSON file validated against [`schema.json`](./schema.json).

Required fields:

- `schema_version`
- `pack_id`
- `title`
- `description`
- `skill_names`

Optional fields:

- `recommended_surface`
- `entrypoints`
- `notes`

## Validation

Validate one or more packs against the schema and the live plugin skill list:

```bash
node scripts/validate_skill_pack.mjs \
  .claude/plugins/minutes/packs/founder-weekly.json \
  .claude/plugins/minutes/packs/relationship-intel.json
```

The validator checks:

- shape against the pack contract
- duplicate skills
- that every referenced skill exists in `.claude/plugins/minutes/plugin.json`

## Import / export model

- **Export**: an agent writes a valid pack JSON file.
- **Import**: another agent reads the pack file, validates it, and then treats
  the `skill_names` list as the canonical skill bundle to recommend or install.

This is intentionally simple and file-backed so Claude Code and Codex can
generate, diff, validate, and share packs without hidden state.
