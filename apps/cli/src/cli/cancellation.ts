export function installCancellationHandlers(
  controller: AbortController,
  forceExit: (status: number) => never = process.exit,
): () => void {
  let interrupted = false;
  const interrupt = () => {
    if (interrupted) forceExit(130);
    interrupted = true;
    controller.abort();
  };
  process.on("SIGINT", interrupt);
  return () => process.off("SIGINT", interrupt);
}
