package common

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
)

type Environment struct {
	Workspace storage.Workspace
	Reset     bool
}

func SetWorkspace(workspaceId uint) tea.Cmd {
	return func() tea.Msg {
		workspace_repo := storage.NewWorkspace()
		workspace, _ := workspace_repo.FindOne(workspaceId)

		return Environment{
			Workspace: workspace,
		}
	}
}

func ResetWorkspace() tea.Cmd {
	return func() tea.Msg {
		return Environment{
			Reset: true,
		}
	}
}
