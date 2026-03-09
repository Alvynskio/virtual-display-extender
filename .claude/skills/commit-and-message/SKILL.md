---
name: commit-and-message
description: Commit current changes and write a CLAUDE.md message to another component's Claude. Use when the user says "commit and message" or wants to send a status update to another sender/receiver.
user-invocable: true
argument-hint: [optional message summary]
---

# Commit and Message

Commit the current changes and write a CLAUDE.md message to one or more other components in this monorepo.

## Step 1: Determine context

Figure out which component you are currently working in by checking the working directory:
- `linux-receiver/` → you are **linux-receiver**
- `win-sender/` → you are **win-sender**
- `mac-sender/` → you are **mac-sender**

The other components are the message recipients. Determine which ones to message based on:
- If the changes are relevant to a specific sender/receiver, message only that one
- If the changes affect the streaming protocol or are broadly relevant, message all other components
- Use your judgement based on what was changed

## Step 2: Commit

1. Run `git status` and `git diff` to review changes
2. Stage the modified files (be specific, don't use `git add -A`)
3. Write a concise commit message following this repo's style (see `git log --oneline -5`)
4. Commit with `Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>`

## Step 3: Write CLAUDE.md message(s)

For each recipient component, write or update `{component}/CLAUDE.md` with:

```markdown
# Message from {your-component} Claude

## What was done
{Summary of changes and why}

## Details
{Technical details the other Claude needs to know — what changed, how it affects them, any action items}

## Status
{Build status, testing notes, anything the user still needs to do}

## If you need anything
{Suggestions for follow-up or what to investigate if issues arise}
```

**Important:**
- If a `CLAUDE.md` already exists in the recipient's directory with prior instructions/tasks, **replace it** — the old task is done, this is the new message
- Be specific and technical — the recipient Claude needs actionable information
- Include what decode chain / pipeline / encoder settings are relevant

## Step 4: Commit the message(s)

Stage and commit the CLAUDE.md file(s) with a message like:
```
message to {recipient} claude on {topic}
```

If `$ARGUMENTS` is provided, incorporate it as context for the message content.
