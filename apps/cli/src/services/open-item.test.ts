import { expect, test } from "bun:test"
import { openItem } from "./open-item.ts"

test("renders the Item context before running an open command", async () => {
  await expect(openItem(
    "test '{{ item.reference_id }}' = 'ABC-123'",
    { item: { reference_id: "ABC-123" } },
    new AbortController().signal,
  )).resolves.toBeUndefined()
})
