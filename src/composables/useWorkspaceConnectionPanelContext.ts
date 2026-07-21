import { inject, provide, type InjectionKey } from "vue";

export type WorkspaceConnectionPanelContext = Record<string, any>;

const WorkspaceConnectionPanelContextKey: InjectionKey<WorkspaceConnectionPanelContext> = Symbol("WorkspaceConnectionPanelContext");

export function provideWorkspaceConnectionPanelContext(context: WorkspaceConnectionPanelContext) {
  provide(WorkspaceConnectionPanelContextKey, context);
}

export function useWorkspaceConnectionPanelContext() {
  const context = inject(WorkspaceConnectionPanelContextKey);
  if (!context) {
    throw new Error("WorkspaceConnectionPanelContext was not provided");
  }
  return context;
}
