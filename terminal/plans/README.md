# Terminal plans

Plan files in this directory use the following name format:

```text
NNN-YYYY-MM-DD-name.md
```

- `NNN` is the plan's unique, zero-padded, three-digit ID.
- `YYYY-MM-DD` is the date of the Git commit that introduces the plan.
- `name` is a short, lowercase, hyphen-separated description.

Example: `018-2026-09-05-example-plan.md`.

## Creating a plan

1. Use the ID in the `NEXT ID` marker below.
2. Use the date on which the plan will be committed.
3. Create the plan file using the required format.
4. Increment `NEXT ID` in the same change. Do not reuse IDs, including IDs from
   plans that were deleted.
5. Before committing, confirm that the filename date matches the commit date and
   update it if necessary.

AI agents that create a plan must follow these steps and keep the marker current.

<!-- NEXT ID: 018 -->
