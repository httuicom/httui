package requests

import (
	"time"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/requests/details"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/common"
	"github.com/gandarfh/httui/pkg/terminal"
	"github.com/gandarfh/httui/pkg/topointer"
	"gorm.io/datatypes"
)

func (m Model) KeyActions(msg tea.KeyMsg) (Model, tea.Cmd) {
	switch {
	case key.Matches(msg, m.keys.Quit):
		return m, tea.Sequence(common.SetState(common.Exit_state), tea.Quit)

	case key.Matches(msg, m.keys.Detail):
		m = m.ShowRequestDetails(msg.String())

		return m, tea.Batch(
			m.Detail.SetRequest(m.Requests.Current),
			tea.Tick(time.Second, func(_ time.Time) tea.Msg {
				return UpdateRequestDefault(m.Requests.Current)
			}),
		)

	case key.Matches(msg, m.keys.OpenGroup):
		m = m.OpenRequest()

	case key.Matches(msg, m.keys.Filter):
		return m, tea.Batch(
			common.OpenCommand("FILTER", ""),
		)

	case key.Matches(msg, m.keys.SetWorkspace):
		return m, tea.Batch(
			common.OpenCommand("SET_WORKSPACE", ""),
		)

	case key.Matches(msg, m.keys.CreateWorkspace):
		return m, tea.Batch(
			common.OpenCommand("CREATE_WORKSPACE", "Type a name for workspace: "),
		)

	case key.Matches(msg, m.keys.Exec):
		if len(m.Requests.List) == 0 {
			return m, nil
		}

		m.Requests.Current = m.List.CurrentNode().Data
		m = m.OpenRequest()

		if m.Requests.Current.Type == "group" {
			break
		}

		return m, tea.Batch(common.SetLoading(true), m.Exec())

	case key.Matches(msg, m.keys.Delete):
		if len(m.Requests.List) == 0 {
			return m, nil
		}

		m.Requests.Current = m.List.CurrentNode().Data

		return m, tea.Batch(
			common.OpenCommand("DELETE", "Type 'Y' to confirm, or any other key to cancel: "),
		)

	case key.Matches(msg, m.keys.Create):
		parentId := m.Requests.Current.ParentID

		if m.Requests.Current.Type == storage.GROUP {
			parentId = &m.Requests.Current.ID
		}

		term := terminal.NewPreview(&storage.Request{
			ParentID: parentId,
		})

		return m, tea.Batch(term.OpenVim("Create"))

	case key.Matches(msg, m.keys.Edit):
		if len(m.Requests.List) == 0 {
			return m, nil
		}

		m.Requests.Current = m.List.CurrentNode().Data
		m = m.OpenRequest()

		if m.Requests.Current.Type == "group" {
			requests, _ := m.RequestsRepo.ListByparent(&m.Requests.Current.ID)

			group := struct {
				Group    storage.Request
				Requests []storage.Request
			}{
				Group:    m.Requests.Current,
				Requests: requests,
			}

			term := terminal.NewPreview(&group)

			return m, tea.Batch(term.OpenVim("Edit"))
		}

		term := terminal.NewPreview(&m.Requests.Current)

		return m, tea.Batch(term.OpenVim("Edit"))

	case key.Matches(msg, m.keys.Envs):
		envs, _ := m.WorkspacesRepo.Environments(m.Workspace.ID)
		term := terminal.NewPreview(&envs)
		return m, tea.Batch(term.OpenVim("Envs"))

	case key.Matches(msg, m.keys.Next):
		cursor := m.Detail.Cursor + 1
		if m.Detail.Cursor == details.CursorPreview {
			cursor = details.CursorTree
		}

		m.keys.DisableKeysForInputs(cursor == details.CursorTree)
		m.List.EnableKeyMap(cursor == details.CursorTree)
		return m, m.Detail.Next()

	case key.Matches(msg, m.keys.Save):
		switch m.Detail.Cursor {
		case details.CursorName:
			name := m.Detail.InputName.Value()
			if name != "" {
				m.Requests.Current.Name = name
				m.RequestsRepo.Update(&m.Requests.Current)
				m.Detail.InputName.Blur()
				m.Detail.Cursor = details.CursorTree
				m.keys.DisableKeysForInputs(true)
				m.List.EnableKeyMap(true)

				return m, tea.Batch(m.LoadRequestsByParentId(m.parentId))
			}

		case details.CursorPreview:
			endpoint := m.Detail.InputPreview.Value()
			if endpoint != "" {
				m.Requests.Current.Endpoint = endpoint
				m.RequestsRepo.Update(&m.Requests.Current)
				m.Detail.InputPreview.Blur()
				m.Detail.Cursor = details.CursorTree
				m.keys.DisableKeysForInputs(true)
				m.List.EnableKeyMap(true)

				return m, tea.Batch(m.LoadRequestsByParentId(m.parentId))
			}
		}
	}

	return m, nil
}

func (m Model) ShowRequestDetails(direction string) Model {
	size := m.List.NumberOfNodes()
	if size == 0 {
		return m
	}

	index := m.List.Index()
	cursor := m.List.Cursor()

	switch direction {
	case "down", "j":
		index += 1
		cursor += 1
	case "up", "k":
		index -= 1
		cursor -= 1
	}

	if size == index || index < 0 {
		index = m.List.Index()
		cursor = m.List.Cursor()
	}

	m.Requests.Current = m.List.GetNodeByIndex(index).Data

	d, _ := m.DefaultsRepo.First()
	d.RequestTree = datatypes.NewJSONType(m.List.Nodes())
	d.Cursor = topointer.New(cursor)
	d.Page = topointer.New(m.List.Paginator.Page)
	d.RequestId = m.Requests.Current.ID
	m.DefaultsRepo.Update(*d)

	if m.Requests.Current.Type == "request" {
		m.parentId = m.Requests.Current.ParentID
	}

	return m
}

func (m Model) OpenRequest() Model {
	if len(m.Requests.List) == 0 {
		return m
	}

	m.List.ToggleExpand()

	m.Requests.Current = m.List.CurrentNode().Data

	if m.Requests.Current.Type == "group" {
		m.previousParentId = m.Requests.Current.ParentID
		m.parentId = &m.Requests.Current.ID
	}

	if m.Requests.Current.Type == "request" {
		m.parentId = m.Requests.Current.ParentID
	}

	d, _ := m.DefaultsRepo.First()
	d.RequestTree = datatypes.NewJSONType(m.List.Nodes())
	d.Cursor = topointer.New(m.List.Cursor())
	d.Page = topointer.New(m.List.Paginator.Page)
	d.RequestId = m.Requests.Current.ID
	m.DefaultsRepo.Update(*d)

	return m
}
