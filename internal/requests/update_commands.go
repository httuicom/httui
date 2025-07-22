package requests

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/common"
)

func (m Model) CommandsActions(msg common.CommandClose) (Model, tea.Cmd) {
	switch msg.Category {
	case "DELETE":
		if strings.ToUpper(msg.Value) == "Y" {
			m.RequestsRepo.Delete(m.Requests.Current.ID)
		}

		return m, tea.Batch(m.LoadRequestsByParentId(m.Requests.Current.ParentID))

	case "FILTER":
		m.filter = msg.Value
		m.List.CursorTop()
		return m, tea.Batch(m.LoadRequestsByFilter(m.filter))

	case "CREATE_WORKSPACE":
		if msg.Value == "" {
			return m, nil
		}

		workspace := storage.Workspace{Name: msg.Value}
		m.WorkspacesRepo.Create(&workspace)
		m.Workspace = workspace

		m.DefaultsRepo.Update(storage.Default{
			WorkspaceId: workspace.ID,
		})

		return m, common.SetWorkspace(m.WorkspacesRepo, workspace.ID)

	case "SET_WORKSPACE":
		if msg.Value == "" {
			return m, nil
		}

		workspace := storage.Workspace{}
		m.WorkspacesRepo.Sql.Model(&workspace).Where("name LIKE ?", "%"+msg.Value+"%").First(&workspace)
		m.Workspace = workspace

		m.DefaultsRepo.Update(storage.Default{
			WorkspaceId: workspace.ID,
		})

		return m, common.SetWorkspace(m.WorkspacesRepo, workspace.ID)
	}

	return m, nil
}
