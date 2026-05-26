import { setupPaneListeners } from "./pane";
import { setupChatListeners, setupSessionWatcher, useChatStore } from "./chat";
import { setupWorkspaceListeners } from "./workspace";
import { useSettingsStore } from "./settings";
import { useEnvironmentStore } from "./environment";

let initialized = false;

export function initTauriBridge() {
  if (initialized) return;
  initialized = true;

  // Setup all Tauri event listeners
  setupPaneListeners();
  setupChatListeners();
  setupWorkspaceListeners();

  // Setup store subscriptions
  setupSessionWatcher();

  // Initialize stores with async data
  useChatStore.getState().initSessions();
  useSettingsStore.getState().loadSettings();
  useEnvironmentStore.getState().refresh();
}
