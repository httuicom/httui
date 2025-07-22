package requests

import (
	"reflect"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/terminal"
	"gorm.io/datatypes"
)

func (m Model) TerminalActions(msg terminal.Finish) (Model, tea.Cmd) {
	switch msg.Category {
	case "Create":
		request := storage.Request{}
		if err := msg.Preview.Execute(&request); err != nil {
			return m, nil
		}

		if request.Name == "" {
			return m, nil
		}

		if request.Type == "" {
			request.Type = storage.REQUEST
		}

		m.RequestsRepo.Create(&request)

		m.Requests.Current = request
		m.parentId = m.Requests.Current.ParentID

		return m, tea.Batch(m.LoadRequestsByParentId(m.parentId))

	case "Edit":
		if m.Requests.Current.Type == "group" {
			var group = struct {
				Group    storage.Request
				Requests []storage.Request
			}{}

			if err := msg.Preview.Execute(&group); err != nil {
				return m, nil
			}

			for _, request := range group.Requests {
				if request.ID == 0 {
					m.RequestsRepo.Create(&request)
				}

				equal := reflect.DeepEqual(request, m.Requests.Current)
				if !equal {
					m.RequestsRepo.Update(&request)
				}
			}

			equal := reflect.DeepEqual(group.Group, m.Requests.Current)
			if !equal {
				m.RequestsRepo.Update(&group.Group)
				m.parentId = group.Group.ParentID

				return m, tea.Batch(m.LoadRequestsByParentId(m.parentId))
			}

			return m, nil
		}

		request := storage.Request{}

		if err := msg.Preview.Execute(&request); err != nil {
			return m, nil
		}

		request.ID = m.Requests.Current.ID

		m.RequestsRepo.Update(&request)
		m.parentId = request.ParentID

		return m, tea.Batch(m.LoadRequestsByParentId(m.parentId))

	case "Envs":
		data := map[string]string{}
		if err := msg.Preview.Execute(&data); err != nil {
			return m, nil
		}

		m.Workspace.Environments = datatypes.NewJSONType(data)
		m.WorkspacesRepo.Update(&m.Workspace)
	}

	defer msg.Preview.Close()
	if msg.Err != nil {
		return m, nil
	}

	return m, nil
}
