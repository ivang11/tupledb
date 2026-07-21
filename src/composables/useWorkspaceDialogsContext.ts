import { inject, provide, type InjectionKey } from "vue";

export type WorkspaceDialogsContext = Record<string, any>;

const WorkspaceDialogsContextKey: InjectionKey<WorkspaceDialogsContext> = Symbol("WorkspaceDialogsContext");

export function provideWorkspaceDialogsContext(context: WorkspaceDialogsContext) {
  provide(WorkspaceDialogsContextKey, context);
}

export function useWorkspaceDialogsContext() {
  const context = inject(WorkspaceDialogsContextKey);
  if (!context) {
    throw new Error("WorkspaceDialogsContext was not provided");
  }
  return context;
}
