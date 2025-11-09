# Spec-Kit Directory

This directory contains Spec-Kit configuration and documentation for the Ceres emulator project.

## Quick Reference

### For AI Agents

1. **Start here**: Read `.specify/memory/constitution.md` - Project principles and standards
2. **Learn workflow**: See `.specify/AGENTS.md` - Detailed workflow guidance
3. **Existing work**: Check `.specify/specs/*/` - See completed and in-progress features

### Key Files

| File                     | Purpose                                                |
| ------------------------ | ------------------------------------------------------ |
| `memory/constitution.md` | Project principles, standards, and governance          |
| `AGENTS.md`              | Spec-Kit workflow guide for AI agents                  |
| `scripts/*.sh`           | Automation scripts for feature management              |
| `templates/*.md`         | Templates for specs, plans, and tasks                  |
| `specs/*/`               | Generated feature specifications (created by commands) |

### Directory Structure

```text
.specify/
├── memory/
│   └── constitution.md          # READ THIS FIRST - Project principles
├── scripts/
│   ├── create-new-feature.sh    # Creates new spec branches
│   ├── setup-plan.sh            # Generates plan artifacts
│   └── update-agent-context.sh  # Updates AGENTS.md references
├── templates/
│   ├── spec-template.md         # Specification template
│   ├── plan-template.md         # Implementation plan template
│   └── tasks-template.md        # Task breakdown template
├── specs/                       # Generated specs (auto-created)
│   └── NNN-feature-name/        # Feature directory
│       ├── spec.md              # Feature specification
│       ├── plan.md              # Implementation plan
│       ├── tasks.md             # Task breakdown
│       ├── research.md          # Technical research
│       ├── data-model.md        # Data structures
│       ├── quickstart.md        # Test scenarios
│       └── contracts/           # API contracts
├── AGENTS.md                    # This file - workflow guide
└── README.md                    # You are here
```

## Available Commands

All commands are available in GitHub Copilot Chat:

### Core Workflow

```text
/speckit.constitution  - Create/review project principles
/speckit.specify       - Create feature specification
/speckit.plan          - Generate implementation plan
/speckit.tasks         - Break down into tasks
/speckit.implement     - Execute implementation
```

### Enhancement Commands

```text
/speckit.clarify       - Ask clarification questions
/speckit.analyze       - Check artifact consistency
/speckit.checklist     - Generate quality checklist
```

## Quick Start

### For New Features

1. Read `memory/constitution.md` to understand project standards
2. Read `AGENTS.md` for workflow guidance
3. Run `/speckit.specify` to create feature spec
4. Run `/speckit.plan` to create implementation plan
5. Run `/speckit.tasks` to generate task breakdown
6. Run `/speckit.implement` to execute tasks

### For Bug Fixes

Same workflow as above, but focus specification on:

- What behavior is broken
- Expected behavior (reference SameBoy/Pan Docs)
- Which tests validate the fix
- Success criteria

## Ceres-Specific Notes

### Reference Materials

- **SameBoy**: Gold standard for behavior (https://github.com/LIJI32/SameBoy)
- **Pan Docs**: Hardware documentation (https://gbdev.io/pandocs/)
- **Test Suite**: Blargg tests in `ceres-test-runner/tests/`

### Test Status

- ✅ cpu_instrs (98% coverage)
- ✅ instr_timing
- ✅ mem_timing
- ❌ mem_timing-2 (needs fix)
- ❌ interrupt_time (needs fix)

### Code Coverage Target

- CPU: Maintain 98%+
- Overall: Target 70%+ (currently ~54%)
- New code: 80%+ coverage required

## More Information

- **Root AGENTS.md**: High-level project overview and architecture
- **Constitution**: `.specify/memory/constitution.md` - Project principles
- **Workflow Guide**: `.specify/AGENTS.md` - Detailed Spec-Kit usage
- **Spec-Kit Docs**: https://github.github.io/spec-kit/

---

**Remember**: Always read the constitution first before creating new specs!
