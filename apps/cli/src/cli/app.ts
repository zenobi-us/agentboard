import { Crust } from "@crustjs/core";

export const app = new Crust("agentboard").meta({
  description: "Collect task-tracking items into local agent work queues",
});
