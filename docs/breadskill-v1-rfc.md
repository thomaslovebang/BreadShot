# BreadSkill v1 RFC

Status: foundation draft implemented by `focusflow-skill-runtime`.

## Product contract

BreadShot records neutral source material and semantic interaction events. A BreadSkill turns
that evidence into an edit plan; the renderer executes the plan. Recording, skill planning and
rendering are separate boundaries.

```text
BreadShot Project + BreadSkill + optional session overrides -> Edit Plan -> Final video
```

Voice and chat assistants are future frontends for selecting a Skill and producing session
overrides. They do not control FFmpeg or platform capture directly.

## Safety model

BreadSkill v1 is data, never executable code. A package cannot run commands, read arbitrary
files, access credentials or make undeclared network requests. Model use is a declared optional
capability and must have a local fallback unless the user deliberately installs a model-required
Skill.

## Package

During the foundation milestone, built-in Skills are validated JSON documents. The public
`.breadskill` container will be a ZIP with this layout:

```text
manifest.json
assets/
  backgrounds/
  sounds/
  fonts/
```

Every referenced asset must stay inside the package. Path traversal and absolute paths are
invalid.

## v1 control surface

- Director: dwell threshold, motion durations, hold durations, zoom per interaction, merge and
  pan rules, confidence threshold.
- Visual: background, canvas padding, corner radius and shadow.
- Cursor: global scale and click effect.
- Editing: idle trimming and threshold.
- Export: aspect ratio, resolution and frame rate.
- Model: required/optional capability and fallback.

The runtime rejects invalid identifiers, unsupported schema versions, unsafe ranges and missing
required model declarations before a Skill can affect a project.

## Compatibility

- `schema_version` describes the BreadSkill document format.
- `id` is a stable lowercase reverse-domain-like identifier.
- `version` describes the author release.
- Projects persist the selected Skill id and version.
- Legacy projects without `selected_skill` remain readable and map their old director profile to
  a built-in Skill during migration.

## Foundation acceptance test

1. The built-in `breadshot.clean` Skill validates.
2. Its director configuration is exactly equal to the pre-Skills Clean defaults.
3. Existing focus-engine tests generate the same timelines.
4. A project saves and reloads the selected Skill.
5. Invalid or unsafe Skills fail before rendering.

## Deferred from v1 foundation

- Community accounts and marketplace ranking.
- Executable plugins.
- Skill signing and remote installation.
- Visual Skill builder.
- Natural-language and voice compilation.
- Skill composition and conflict resolution.

Those features depend on the schema and edit-plan boundary proving stable first.
