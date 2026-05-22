import { setupPaneListeners } from "./pane";
import { setupChatListeners, setupSessionWatcher, useChatStore } from "./chat";
import { setupWorkspaceListeners } from "./workspace";
import { useSettingsStore } from "./settings";
import { useEnvironmentStore } from "./environment";

let initialized = false;

export function initTauriBridge() {
  if (initialized) return;
  initialized = true;

  setupPaneListeners();
  setupChatListeners();
  setupWorkspaceListeners();

  setupSessionWatcher();

  useChatStore.getState().initSessions();
  useSettingsStore.getState().loadSettings();
  useEnvironmentStore.getState().refresh();
}
