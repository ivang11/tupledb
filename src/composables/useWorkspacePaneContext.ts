import { inject, provide, type InjectionKey } from "vue";

export type WorkspacePaneContext = Record<string, any>;

const WorkspacePaneContextKey: InjectionKey<WorkspacePaneContext> = Symbol("WorkspacePaneContext");

export function provideWorkspacePaneContext(context: WorkspacePaneContext) {
  provide(WorkspacePaneContextKey, context);
}

export function useWorkspacePaneContext() {
  const context = inject(WorkspacePaneContextKey);
  if (!context) {
    throw new Error("WorkspacePaneContext was not provided");
  }
  return context;
}
